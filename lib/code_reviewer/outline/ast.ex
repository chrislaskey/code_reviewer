defmodule CodeReviewer.Outline.AST do
  @moduledoc """
  Exact outline parser built on `Code.string_to_quoted/2`, zero dependencies.

  Requires syntactically complete source. With `token_metadata: true` the
  AST carries `line` and `end: [line: n]` metadata for every `do ... end`
  block, which is all an outline needs. Keyword `do:` bodies have no `end`
  metadata, so their last line is the deepest line found in the body.

  For error-tolerant parsing of fragments, `Spitfire` returns the same AST
  shape from broken input and could be dropped in here later.
  """

  alias CodeReviewer.Outline
  alias CodeReviewer.Outline.{Directive, Module}

  @definers Outline.definers()
  @directives ~w(alias import use require attr slot)a
  @attributes ~w(moduledoc behaviour derive type typep opaque callback macrocallback)a
  @annotations ~w(doc spec impl deprecated)a
  @wrappers ~w(describe if unless)a

  @doc "Parses `source`; `{:error, reason}` when the source does not parse."
  @spec parse(String.t()) :: {:ok, [Module.t()]} | {:error, term()}
  def parse(source) when is_binary(source) do
    case Code.string_to_quoted(source, columns: true, token_metadata: true, unescape: false) do
      {:ok, ast} -> {:ok, ast |> collect_modules([]) |> Enum.reverse()}
      {:error, reason} -> {:error, reason}
    end
  rescue
    e -> {:error, e}
  end

  # -- modules -----------------------------------------------------------------

  defp collect_modules({:defmodule, meta, [alias_ast, [do: body]]}, acc) do
    name = module_name(alias_ast)
    line_start = meta[:line]
    line_end = end_line(meta, body)
    {clauses, directives, nested} = walk_body(body)

    module = %Module{
      name: name,
      line_start: line_start,
      line_end: line_end,
      functions: Outline.group_clauses(clauses),
      directives: Enum.reverse(directives)
    }

    # nested modules are reported after their parent, with fully qualified names
    Enum.reduce(nested, [module | acc], fn nested_ast, a ->
      nested_ast
      |> collect_modules([])
      |> Enum.map(&qualify(&1, name))
      |> Enum.reverse()
      |> Kernel.++(a)
    end)
  end

  defp collect_modules({:__block__, _, items}, acc),
    do: Enum.reduce(items, acc, &collect_modules/2)

  defp collect_modules(_, acc), do: acc

  defp qualify(%Module{name: nested} = m, parent) when is_binary(nested) do
    if String.starts_with?(nested, "__MODULE__"),
      do: %{m | name: String.replace_prefix(nested, "__MODULE__", parent)},
      else: %{m | name: parent <> "." <> nested}
  end

  defp qualify(m, _parent), do: m

  defp module_name({:__aliases__, _, parts}), do: Enum.map_join(parts, ".", &part_to_string/1)
  defp module_name(other), do: Macro.to_string(other)

  defp part_to_string({:__MODULE__, _, _}), do: "__MODULE__"
  defp part_to_string(atom) when is_atom(atom), do: Atom.to_string(atom)
  defp part_to_string(other), do: Macro.to_string(other)

  # -- module body -------------------------------------------------------------

  defp walk_body({:__block__, _, items}), do: walk_items(items, {[], [], []}, [])
  defp walk_body(single), do: walk_items([single], {[], [], []}, [])

  # `pending` holds @doc/@spec/@impl seen since the last def
  defp walk_items([], {clauses, directives, nested}, _pending),
    do: {Enum.reverse(clauses), directives, Enum.reverse(nested)}

  defp walk_items([item | rest], {clauses, directives, nested}, pending) do
    case item do
      {kind, meta, [head | body]} when kind in @definers ->
        clause = clause(kind, meta, head, body, pending)
        walk_items(rest, {[clause | clauses], directives, nested}, [])

      {:defmodule, _, _} = nested_mod ->
        walk_items(rest, {clauses, directives, [nested_mod | nested]}, pending)

      {:test, meta, [name_ast | body]} when is_binary(name_ast) ->
        clause = %{
          kind: :test,
          name: name_ast,
          arity: nil,
          head: "test #{inspect(name_ast)}",
          line_start: meta[:line],
          line_end: clause_end(meta, body),
          annotations: pending
        }

        walk_items(rest, {[clause | clauses], directives, nested}, [])

      {:@, meta, [{attr, _, [value | _]}]} when attr in @annotations ->
        ann = %{kind: :"@#{attr}", text: short_text(value), line: meta[:line]}
        walk_items(rest, {clauses, directives, nested}, pending ++ [ann])

      {:@, meta, [{attr, _, [value | _]}]} when attr in @attributes ->
        directive = %Directive{kind: attr, text: short_text(value), line: meta[:line]}
        walk_items(rest, {clauses, [directive | directives], nested}, pending)

      {kind, meta, args} when kind in @directives and is_list(args) ->
        directive = %Directive{kind: kind, text: args_text(args), line: meta[:line]}
        walk_items(rest, {clauses, [directive | directives], nested}, pending)

      {kind, meta, args} when kind in [:defstruct, :defexception] ->
        directive = %Directive{kind: kind, text: args_text(args || []), line: meta[:line]}
        walk_items(rest, {clauses, [directive | directives], nested}, pending)

      # ExUnit `describe` and compile-time `if`/`unless` wrap ordinary defs
      {wrapper, _, args} when wrapper in @wrappers and is_list(args) ->
        inner = args |> List.last() |> block_items()
        walk_items(inner ++ rest, {clauses, directives, nested}, pending)

      _ ->
        walk_items(rest, {clauses, directives, nested}, pending)
    end
  end

  defp block_items(blocks) when is_list(blocks) do
    Enum.flat_map(blocks, fn
      {key, {:__block__, _, items}} when key in [:do, :else] -> items
      {key, single} when key in [:do, :else] -> [single]
      _ -> []
    end)
  end

  defp block_items(_), do: []

  # -- clauses -----------------------------------------------------------------

  defp clause(kind, meta, head, body, pending) do
    {name, args} = name_and_args(head)

    %{
      kind: kind,
      name: name,
      arity: length(args),
      head: head |> Macro.to_string() |> single_line(),
      line_start: meta[:line],
      line_end: clause_end(meta, body),
      annotations: pending
    }
  end

  defp name_and_args({:when, _, [inner | _guards]}), do: name_and_args(inner)

  defp name_and_args({name, _, args}) when is_atom(name) and is_list(args),
    do: {Atom.to_string(name), args}

  defp name_and_args({name, _, nil}) when is_atom(name), do: {Atom.to_string(name), []}
  # def unquote(name)(args)
  defp name_and_args({{:unquote, _, _} = u, _, args}) when is_list(args),
    do: {Macro.to_string(u), args}

  defp name_and_args(other), do: {Macro.to_string(other), []}

  # `do ... end` bodies carry `end` metadata. `do:` keyword bodies do not, but
  # `end_of_expression` marks the line the whole definition finished on
  # (absent only for the very last expression before a closing `end`, where
  # the deepest line found in the body is the fallback).
  defp clause_end(meta, body) do
    Keyword.get(meta[:end] || [], :line) ||
      Keyword.get(meta[:end_of_expression] || [], :line) ||
      Enum.max([meta[:line] | Enum.map(body, &max_line/1)])
  end

  defp end_line(meta, body) do
    case Keyword.get(meta[:end] || [], :line) do
      nil -> max_line(body)
      line -> line
    end
  end

  defp max_line(ast) do
    {_, max} =
      Macro.prewalk(ast, 0, fn
        {_, meta, _} = node, acc when is_list(meta) ->
          lines = [
            meta[:line],
            get_in(meta, [:end, :line]),
            get_in(meta, [:closing, :line]),
            get_in(meta, [:end_of_expression, :line])
          ]

          {node, Enum.max([acc | Enum.reject(lines, &is_nil/1)])}

        node, acc ->
          {node, acc}
      end)

    max
  end

  # -- text helpers --------------------------------------------------------------

  defp args_text(args), do: args |> Enum.map_join(", ", &Macro.to_string/1) |> single_line()

  defp short_text(value) when is_binary(value) do
    value
    |> String.split("\n", trim: true)
    |> List.first("")
    |> String.trim()
    |> String.slice(0, 80)
  end

  defp short_text(value), do: value |> Macro.to_string() |> single_line() |> String.slice(0, 120)

  defp single_line(text), do: text |> String.split("\n") |> Enum.map_join(" ", &String.trim/1)
end
