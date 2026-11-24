defmodule EctoRust.Source do
  @moduledoc """
  Finds the changeset functions marked `@universal_changeset true` in a
  module's source file, the way `HeexSvelte.Source` finds
  `@universal_component true` components.

  Each marked function comes with its clauses as quoted Elixir, for the
  transpiler, along with every other def of the module, so a helper the
  function pipes into can be followed, and the module attributes, so
  `@statuses` in a validation resolves to its list.
  """

  defstruct [:module, :function, :arity, :file, :line, :clauses, :defs, :attributes]

  @doc """
  Returns the marked functions of `module`, in source order. A module
  without a source file, or with no mark, has none.
  """
  def marked(module) do
    case read(module) do
      {:ok, file, statements} ->
        for signature <- marked_names(statements), do: source(module, file, statements, signature)

      :error ->
        []
    end
  end

  @doc """
  The function `name/arity` of `module` as a source, marked or not, so the
  transpiler can be tried on any function.
  """
  def function(module, name, arity) do
    with {:ok, file, statements} <- read(module),
         true <- Map.has_key?(defs(statements), {name, arity}) do
      {:ok, source(module, file, statements, {name, arity})}
    else
      _ -> :error
    end
  end

  defp read(module) do
    file = module.module_info(:compile)[:source] |> to_string() |> Path.relative_to_cwd()

    with {:ok, contents} <- File.read(file),
         {:ok, quoted} <- Code.string_to_quoted(contents, file: file),
         {:ok, body} <- module_body(quoted, module) do
      {:ok, file, statements(body)}
    else
      _ -> :error
    end
  end

  defp source(module, file, statements, {name, arity}) do
    defs = defs(statements)
    clauses = Map.fetch!(defs, {name, arity})
    {_kind, meta, _} = hd(clauses)

    %__MODULE__{
      module: module,
      function: name,
      arity: arity,
      file: file,
      line: meta[:line],
      clauses: clauses,
      defs: defs,
      attributes: attributes(statements)
    }
  end

  defp module_body(quoted, module, parent \\ nil) do
    quoted
    |> statements()
    |> Enum.find_value(:error, fn
      {:defmodule, _, [{:__aliases__, _, parts}, [do: body]]} ->
        name = if parent, do: Module.concat([parent | parts]), else: Module.concat(parts)
        if name == module, do: {:ok, body}, else: with(:error <- module_body(body, module, name), do: nil)

      _statement ->
        nil
    end)
  end

  defp statements({:__block__, _, statements}), do: statements
  defp statements(statement), do: [statement]

  # The def that follows each `@universal_changeset true`
  defp marked_names(statements) do
    {marked, _pending} =
      Enum.reduce(statements, {[], false}, fn statement, {marked, pending} ->
        case {statement, def_signature(statement)} do
          {{:@, _, [{:universal_changeset, _, [true]}]}, _} -> {marked, true}
          {_, nil} -> {marked, pending}
          {_, signature} when pending -> {[signature | marked], false}
          _ -> {marked, false}
        end
      end)

    Enum.reverse(marked)
  end

  # Every def and defp of the module, as `{name, arity} => clauses` in
  # source order. Default arguments count as present, so `def f(a, b, c \\ [])`
  # is `{:f, 3}` only.
  defp defs(statements) do
    statements
    |> Enum.filter(&def_signature/1)
    |> Enum.group_by(&def_signature/1)
  end

  defp attributes(statements) do
    for {:@, _, [{name, _, [value]}]} <- statements, is_atom(name), into: %{}, do: {name, value}
  end

  defp def_signature({kind, _, [head | _]}) when kind in [:def, :defp] do
    case head do
      {:when, _, [{name, _, args} | _]} -> {name, length(args)}
      {name, _, args} when is_list(args) -> {name, length(args)}
      {name, _, nil} -> {name, 0}
    end
  end

  defp def_signature(_statement), do: nil
end
