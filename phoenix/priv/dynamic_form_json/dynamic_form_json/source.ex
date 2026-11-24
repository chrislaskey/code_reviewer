defmodule DynamicFormJson.Source do
  @moduledoc """
  Finds the functions marked `@universal_dynamic_form true` in a module's
  source file.

  A marked function is one public `def` clause with no arguments, since
  `mix app.dynamic_forms` calls it once to write its file. Anything else is
  reported with the file and line.
  """

  defstruct [:module, :function, :file, :line]

  @doc """
  Returns `{forms, errors}` for `module`, each error a map of `:file`,
  `:line` and `:message`.
  """
  def forms(module) do
    file = module.module_info(:compile)[:source] |> to_string() |> Path.relative_to_cwd()

    with {:ok, source} <- File.read(file),
         {:ok, quoted} <- Code.string_to_quoted(source, file: file),
         {:ok, body} <- module_body(quoted, module) do
      body
      |> statements()
      |> marked_defs()
      |> Enum.map(&form(&1, module, file))
      |> Enum.split_with(&match?(%__MODULE__{}, &1))
    else
      _ -> {[], []}
    end
  end

  # Finds `defmodule` for `module`, nested ones included: `defmodule B`
  # inside `defmodule A` defines `A.B`, as in Elixir
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

  # Pairs each `@universal_dynamic_form true` with the def after it, and
  # keeps the clauses that follow with the same name and arity, to report
  # them
  defp marked_defs(statements) do
    {marked, _pending} =
      Enum.reduce(statements, {[], false}, fn statement, {marked, pending} ->
        case {statement, head(statement), marked} do
          {{:@, _, [{:universal_dynamic_form, _, [true]}]}, _, _} ->
            {marked, true}

          {_, nil, _} ->
            {marked, pending}

          {_, {name, args}, _} when pending ->
            {[{{name, length(args)}, [statement]} | marked], false}

          {_, {name, args}, [{{name, arity}, defs} | rest]} when length(args) == arity ->
            {[{{name, arity}, [statement | defs]} | rest], false}

          _ ->
            {marked, false}
        end
      end)

    Enum.reverse(marked)
  end

  defp head({kind, _, [head | _]}) when kind in [:def, :defp] do
    case head do
      {:when, _, [{name, _, args} | _]} -> {name, args || []}
      {name, _, args} -> {name, args || []}
    end
  end

  defp head(_statement), do: nil

  # `defs` are the function's clauses, last first
  defp form({{function, _arity}, defs}, module, file) do
    [{kind, meta, _} = def | _] = Enum.reverse(defs)
    {_name, args} = head(def)

    problem =
      cond do
        kind == :defp -> "a marked form is public, def, for `mix app.dynamic_forms` to call it"
        args != [] -> "a marked form takes no arguments: fill in what changes at runtime after calling it"
        length(defs) > 1 -> "a marked form is one clause"
        true -> nil
      end

    if problem,
      do: %{file: file, line: meta[:line], message: "#{function}: #{problem}"},
      else: %__MODULE__{module: module, function: function, file: file, line: meta[:line]}
  end
end
