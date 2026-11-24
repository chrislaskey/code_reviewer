defmodule EctoRust.Transpile do
  @moduledoc """
  Writes a marked changeset function as Rust, from its source: a pipeline
  of `cast/3`, `validate_*` and `put_change/3` calls with literal arguments
  becomes the same chain of calls on the `ecto_changeset` crate's
  `Changeset`.

  The function's body pipes its first argument through `Ecto.Changeset`
  functions. A pipe into another function of the module is followed when
  that function is such a pipeline too; a function with several clauses is
  followed when every clause gives the same Rust, as a helper that puts a
  change or leaves the changeset alone does. Module attributes such as
  `@statuses` resolve to their value.

  `put_change/3` writes nothing: the app sets such a field, the owner
  from the scope or the sync time, and the Rust reads it from `data`,
  which is how `App.Data.UniversalChangeset.run/3` and the other apps pass
  it. So a put field may be required but not otherwise validated, and may
  not be cast. Anything else, such as an `if`, a `case`, a call with an
  argument that is not a literal, or a helper with logic of its own,
  fails with its file and line.
  """

  alias EctoRust.Source

  @validations ~w(validate_required validate_length validate_format validate_number validate_inclusion
    validate_exclusion validate_subset validate_confirmation validate_acceptance)a

  @builtin_types ~w(id string integer float decimal boolean binary date time utc_datetime
    utc_datetime_usec naive_datetime naive_datetime_usec map)a

  @doc """
  The Rust source for the marked functions of one module, as
  `{:ok, rust}` or `{:error, problems}`.
  """
  def module([%Source{module: module, file: file} | _] = sources) do
    with {:ok, functions} <- functions(sources),
         {:ok, fields} <- fields(module, file, functions) do
      {:ok, module |> render(file, fields, functions) |> format()}
    end
  end

  @doc """
  The steps of each marked function: `{:ok, functions}` with the function's
  name, its steps, and the fields it casts and the app sets, or
  `{:error, problems}`.
  """
  def functions(sources) do
    case Enum.map(sources, &function/1) |> Enum.split_with(&match?({:ok, _}, &1)) do
      {functions, []} -> {:ok, Enum.map(functions, fn {:ok, function} -> function end)}
      {_, errors} -> {:error, Enum.flat_map(errors, fn {:error, problems} -> problems end)}
    end
  end

  defp function(%Source{} = source) do
    with {:ok, steps} <- steps(source, source.function, source.arity, [], []),
         :ok <- check_app_set(source, steps) do
      cast =
        Enum.flat_map(steps, fn
          {:cast, fields} -> fields
          _ -> []
        end)

      app_set = for {:app_set, field} <- steps, do: field
      {:ok, %{name: source.function, steps: steps, cast: cast, app_set: app_set}}
    else
      {:error, line, message} ->
        {:error,
         [%{file: source.file, line: line, message: "#{source.function}/#{source.arity}: #{message}"}]}
    end
  end

  # A put field is data to the Rust: it may be required, and nothing else
  defp check_app_set(source, steps) do
    cast =
      Enum.flat_map(steps, fn
        {:cast, fields} -> fields
        _ -> []
      end)

    app_set = for {:app_set, field} <- steps, do: field

    validated =
      for {kind, field, _args} <- steps, kind in @validations, kind != :validate_required, do: {kind, field}

    case {Enum.find(app_set, &(&1 in cast)), Enum.find(validated, fn {_kind, field} -> field in app_set end)} do
      {nil, nil} ->
        :ok

      {field, _} when field != nil ->
        {:error, source.line,
         "#{inspect(field)} is cast and put, so the Rust cannot tell which value to validate"}

      {nil, {kind, field}} ->
        {:error, source.line,
         "#{kind}(#{inspect(field)}) validates a field the app puts, which the Rust reads as data"}
    end
  end

  # The steps of `name/arity`, a pipeline on its first argument. `args`
  # are the caller's arguments when the function is piped into from
  # another, bound to the parameter names. Every clause must give the
  # same steps.
  defp steps(source, name, arity, args, stack) do
    with :ok <- not_recursive(name, arity, stack),
         {:ok, clauses} <- clauses(source, name, arity) do
      results =
        Enum.map(clauses, fn {head, body} -> clause_steps(source, name, arity, head, body, args, stack) end)

      case Enum.find(results, &match?({:error, _, _}, &1)) do
        nil -> same_rules(results, name, arity, source.line)
        error -> error
      end
    end
  end

  # Every clause must give the same Rust. A `put_change` writes none, so
  # a clause that puts a field and one that leaves the changeset alone
  # agree, and the put fields of every clause are kept
  defp same_rules(results, name, arity, line) do
    without_puts = Enum.map(results, fn {:ok, steps} -> Enum.reject(steps, &match?({:app_set, _}, &1)) end)

    case Enum.uniq_by(without_puts, &comparable/1) do
      [steps] ->
        puts =
          results
          |> Enum.flat_map(fn {:ok, steps} -> steps end)
          |> Enum.filter(&match?({:app_set, _}, &1))
          |> Enum.uniq()

        {:ok, steps ++ puts}

      _ ->
        {:error, line, "the clauses of #{name}/#{arity} give different rules"}
    end
  end

  defp clause_steps(source, name, arity, head, body, args, stack) do
    with {:ok, [first | params]} <- arg_names(head) do
      bindings = for {param, arg} <- Enum.zip(params, args), param, into: %{}, do: {param, arg}
      body = body |> resolve_attributes(source.attributes) |> bind(bindings)

      case unpipe(body, first) do
        [{^first, _, context} | calls] when is_atom(context) ->
          Enum.reduce_while(calls, {:ok, []}, fn call, {:ok, acc} ->
            case step(source, call, [{name, arity} | stack]) do
              {:ok, steps} -> {:cont, {:ok, acc ++ steps}}
              error -> {:halt, error}
            end
          end)

        [other | _] ->
          {:error, line(other, head),
           "#{name}/#{arity} must pipe `#{first}` through Ecto.Changeset functions"}
      end
    end
  end

  # A regex compiled twice is two terms, so steps compare by its source
  # Steps are data, not AST: a walk with `Macro.prewalk/2` would read a
  # step like `{:validate_format, field, {regex, opts}}` as a call
  defp comparable(%Regex{} = regex), do: {Regex.source(regex), Regex.opts(regex)}
  defp comparable(list) when is_list(list), do: Enum.map(list, &comparable/1)
  defp comparable(tuple) when is_tuple(tuple), do: tuple |> Tuple.to_list() |> comparable() |> List.to_tuple()
  defp comparable(other), do: other

  defp not_recursive(name, arity, stack) do
    if {name, arity} in stack, do: {:error, 0, "#{name}/#{arity} pipes into itself"}, else: :ok
  end

  defp clauses(source, name, arity) do
    case Map.get(source.defs, {name, arity}) do
      nil ->
        {:error, source.line, "#{name}/#{arity} is not defined in the module"}

      clauses ->
        Enum.reduce_while(clauses, {:ok, []}, fn
          {_kind, _meta, [head, [do: body]]}, {:ok, acc} ->
            {:cont, {:ok, acc ++ [{head, body}]}}

          {_kind, meta, _}, _ ->
            {:halt, {:error, meta[:line], "#{name}/#{arity} has a clause without a do block"}}
        end)
    end
  end

  # The first argument must be a variable, the changeset. The others are
  # variables, bound to the caller's arguments, or patterns such as `nil`
  # that bind nothing
  defp arg_names({:when, _, [head | _]}), do: arg_names(head)

  defp arg_names({_name, meta, [first | rest]}) do
    case name(first) do
      nil -> {:error, meta[:line], "the first argument must be a plain variable"}
      first -> {:ok, [first | Enum.map(rest, &name/1)]}
    end
  end

  defp name({:\\, _, [arg, _default]}), do: name(arg)
  defp name({name, _, context}) when is_atom(name) and is_atom(context), do: name
  defp name(_pattern), do: nil

  defp resolve_attributes(ast, attributes) do
    Macro.prewalk(ast, fn
      {:@, _, [{name, _, nil}]} = node -> Map.get(attributes, name, node)
      node -> node
    end)
  end

  defp bind(ast, bindings) when map_size(bindings) == 0, do: ast

  defp bind(ast, bindings) do
    Macro.prewalk(ast, fn
      {name, _, context} = node when is_atom(context) -> Map.get(bindings, name, node)
      node -> node
    end)
  end

  # `f(post, params)` as a body is `post |> f(params)`
  defp unpipe({name, meta, [{first, _, context} = var | args]}, first)
       when is_atom(name) and is_atom(context) and name != :|> do
    [var, {name, meta, args}]
  end

  defp unpipe(body, _first), do: unpipe(body)

  defp unpipe({:|>, _, [left, right]}), do: unpipe(left) ++ [right]
  defp unpipe(other), do: [other]

  defp step(_source, {:cast, meta, [_attrs, fields | _opts]}, _stack) do
    with {:ok, fields} <- literal(fields, meta), do: {:ok, [{:cast, fields}]}
  end

  defp step(_source, {:validate_required, meta, [fields | opts]}, _stack) do
    with {:ok, fields} <- literal(fields, meta),
         {:ok, opts} <- literal(List.first(opts, []), meta) do
      {:ok, for(field <- List.wrap(fields), do: {:validate_required, field, opts})}
    end
  end

  defp step(_source, {:validate_format, meta, [field, regex | opts]}, _stack) do
    with {:ok, field} <- literal(field, meta),
         {:ok, regex} <- literal(regex, meta),
         {:ok, opts} <- literal(List.first(opts, []), meta) do
      {:ok, [{:validate_format, field, {regex, opts}}]}
    end
  end

  defp step(_source, {kind, meta, [field, data | opts]}, _stack)
       when kind in [:validate_inclusion, :validate_exclusion, :validate_subset] do
    with {:ok, field} <- literal(field, meta),
         {:ok, data} <- literal(data, meta),
         {:ok, opts} <- literal(List.first(opts, []), meta),
         true <-
           is_list(data) ||
             {:error, meta[:line], "#{kind} takes a list here: Ecto keeps a range in the error"} do
      {:ok, [{kind, field, {data, opts}}]}
    end
  end

  defp step(_source, {kind, meta, [field | opts]}, _stack) when kind in @validations do
    with {:ok, field} <- literal(field, meta),
         {:ok, opts} <- literal(List.first(opts, []), meta) do
      {:ok, [{kind, field, opts}]}
    end
  end

  defp step(_source, {:put_change, meta, [field, _value]}, _stack) do
    with {:ok, field} <- literal(field, meta), do: {:ok, [{:app_set, field}]}
  end

  defp step(_source, {:unique_constraint, meta, [fields | opts]}, _stack) do
    with {:ok, fields} <- literal(fields, meta),
         {:ok, opts} <- literal(List.first(opts, []), meta) do
      {:ok, [{:constraint, "unique", List.wrap(fields), opts}]}
    end
  end

  defp step(_source, {:unsafe_validate_unique, meta, [fields | _opts]}, _stack) do
    with {:ok, fields} <- literal(fields, meta) do
      {:ok, [{:note, "unsafe_validate_unique(#{inspect(fields)}) queries the database and is not mirrored"}]}
    end
  end

  defp step(source, {name, meta, args}, stack) when is_atom(name) and is_list(args) do
    arity = length(args) + 1

    if Map.has_key?(source.defs, {name, arity}) do
      steps(source, name, arity, args, stack)
    else
      {:error, meta[:line],
       "#{name}/#{arity} is not a mirrored Ecto.Changeset function or a function of the module"}
    end
  end

  defp step(_source, other, _stack) do
    {:error, line(other, nil), "cannot follow `#{Macro.to_string(other) |> String.slice(0, 40)}`"}
  end

  # A literal argument, evaluated: lists, keywords, strings, numbers,
  # ranges and `~r` sigils. A variable or a call fails.
  defp literal(ast, meta) do
    if Macro.quoted_literal?(ast) or sigil_or_range?(ast) or keyword_literal?(ast) do
      {value, _binding} = Code.eval_quoted(ast)
      {:ok, value}
    else
      {:error, meta[:line], "`#{Macro.to_string(ast)}` is not a literal"}
    end
  rescue
    error ->
      {:error, meta[:line], "`#{Macro.to_string(ast)}` cannot be evaluated: #{Exception.message(error)}"}
  end

  defp sigil_or_range?({:sigil_r, _, _}), do: true
  defp sigil_or_range?({:.., _, [a, b]}), do: Macro.quoted_literal?(a) and Macro.quoted_literal?(b)
  defp sigil_or_range?(_), do: false

  defp keyword_literal?(list) when is_list(list) do
    Enum.all?(list, fn
      {key, value} when is_atom(key) -> Macro.quoted_literal?(value) or sigil_or_range?(value)
      value -> Macro.quoted_literal?(value) or sigil_or_range?(value)
    end)
  end

  defp keyword_literal?(_), do: false

  defp line(ast, fallback), do: meta_line(ast) || meta_line(fallback) || 0

  defp meta_line({_, meta, _}) when is_list(meta), do: meta[:line]
  defp meta_line(_ast), do: nil

  ## Rust

  # The type of every cast field, as the crate casts it
  defp fields(module, file, functions) do
    types = module.__changeset__()

    functions
    |> Enum.flat_map(& &1.cast)
    |> Enum.uniq()
    |> Enum.sort()
    |> Enum.reduce_while({:ok, []}, fn field, {:ok, acc} ->
      case rust_type(Map.get(types, field)) do
        {:ok, type} ->
          {:cont, {:ok, acc ++ [{field, type}]}}

        :error ->
          {:halt,
           {:error,
            [
              %{
                file: file,
                line: 0,
                message: "the crate cannot cast #{inspect(field)}, of type #{inspect(types[field])}"
              }
            ]}}
      end
    end)
  end

  defp rust_type({:array, type}) do
    with {:ok, inner} <- rust_type(type), do: {:ok, "FieldType::Array(Box::new(#{inner}))"}
  end

  defp rust_type(type) when type in @builtin_types,
    do: {:ok, "FieldType::#{Macro.camelize(Atom.to_string(type))}"}

  defp rust_type(Ecto.UUID), do: {:ok, "FieldType::Uuid"}

  defp rust_type({:parameterized, {Ecto.Enum, %{mappings: mappings}}}) do
    values = Enum.map_join(mappings, ", ", fn {name, _value} -> "#{rust_string(name)}.to_string()" end)
    {:ok, "FieldType::Enum(vec![#{values}])"}
  end

  defp rust_type(_type), do: :error

  defp render(module, file, fields, functions) do
    """
    //! Generated by `mix app.rust` from #{file}.
    //! Do not edit: change the changeset function and run `mix app.rust`.
    #![allow(unused_imports)]

    use std::collections::BTreeMap;

    use ecto_changeset::{Changeset, Constraint, Count, FieldType, Length, NumberCheck, Params};
    use serde_json::{Number, Value, json};

    /// The Ecto type of every field the changeset functions cast.
    pub fn fields() -> BTreeMap<String, FieldType> {
        BTreeMap::from([
    #{Enum.map_join(fields, "\n", fn {field, type} -> "        (#{rust_string(field)}.to_string(), #{type})," end)}
        ])
    }
    #{Enum.map_join(functions, "\n", &render_function(module, &1))}
    """
    |> String.replace(~r/\n{3,}/, "\n\n")
  end

  @doc """
  Formats Rust with `rustfmt` when it is installed, so the file is what
  `cargo fmt` would write. Without it the file is written as rendered, and
  shows as out of date on a machine that has it. The file is formatted in
  `dir`, the directory it is written to, so a `mod` included by a relative
  path resolves.
  """
  def format(rust, dir \\ System.tmp_dir!()) do
    case System.find_executable("rustfmt") do
      nil ->
        rust

      rustfmt ->
        path = Path.join(dir, "ecto_rust_#{System.unique_integer([:positive])}.rs")
        File.write!(path, rust)

        try do
          case System.cmd(rustfmt, ["--edition", "2024", "--quiet", path], stderr_to_stdout: true) do
            {_, 0} -> File.read!(path)
            {output, _status} -> raise "rustfmt failed on the generated Rust: #{output}"
          end
        after
          File.rm(path)
        end
    end
  end

  defp render_function(module, %{name: name, steps: steps, app_set: app_set}) do
    {constraints, steps} = Enum.split_with(steps, &match?({:constraint, _, _, _}, &1))
    {notes, steps} = Enum.split_with(steps, &match?({:note, _}, &1))
    steps = steps |> Enum.reject(&match?({:app_set, _}, &1)) |> group_required()

    app_set_note =
      case app_set do
        [] -> ""
        fields -> "/// The app sets #{Enum.map_join(fields, " and ", &"`#{&1}`")}, so pass them in `data`.\n"
      end

    """

    /// `#{inspect(module)}.#{name}`.
    #{app_set_note}#{Enum.map_join(notes, "", fn {:note, text} -> "/// #{text}\n" end)}pub fn #{name}(data: &Params, params: &Params) -> Changeset {
    #{Enum.map_join(steps, "\n", &render_step/1)}
    }

    /// The constraints `#{name}` leaves to the database.
    pub fn #{name}_constraints() -> Vec<Constraint> {
        vec![#{Enum.map_join(constraints, ", ", &render_constraint/1)}]
    }
    """
  end

  # `validate_required([:a, :b])` is one step per field, so a put field
  # can be told apart, and one call again in the Rust
  defp group_required([{:validate_required, field, opts} | rest]) do
    {same, rest} = Enum.split_while(rest, &match?({:validate_required, _, ^opts}, &1))
    [{:required, [field | Enum.map(same, &elem(&1, 1))]} | group_required(rest)]
  end

  defp group_required([step | rest]), do: [step | group_required(rest)]
  defp group_required([]), do: []

  defp render_step({:cast, fields}),
    do: "    Changeset::cast(data, &fields(), params, &[#{rust_strings(fields)}])"

  defp render_step({:required, fields}), do: "        .validate_required(&[#{rust_strings(fields)}])"

  defp render_step({:validate_format, field, {regex, opts}}),
    do: "        .validate_format(#{rust_string(field)}, #{rust_regex(regex)}, #{rust_message(opts)})"

  defp render_step({:validate_length, field, opts}) do
    length =
      Enum.map_join(Keyword.take(opts, [:is, :min, :max, :count]), "", fn
        {:count, count} -> ".count(Count::#{Macro.camelize(Atom.to_string(count))})"
        {key, value} -> ".#{key}(#{value})"
      end)

    "        .validate_length(#{rust_string(field)}, &Length::default()#{length}, #{rust_message(opts)})"
  end

  defp render_step({:validate_number, field, opts}) do
    checks =
      opts
      |> Keyword.delete(:message)
      |> Enum.map_join(", ", fn {op, value} ->
        "NumberCheck::#{Macro.camelize(Atom.to_string(op))}(#{rust_number(value)})"
      end)

    "        .validate_number(#{rust_string(field)}, &[#{checks}], #{rust_message(opts)})"
  end

  defp render_step({kind, field, {values, opts}})
       when kind in [:validate_inclusion, :validate_exclusion, :validate_subset] do
    values = Enum.map_join(values, ", ", &"json!(#{rust_json(&1)})")
    "        .#{kind}(#{rust_string(field)}, &[#{values}], #{rust_message(opts)})"
  end

  defp render_step({:validate_confirmation, field, opts}),
    do:
      "        .validate_confirmation(#{rust_string(field)}, #{opts[:required] || false}, #{rust_message(opts)})"

  defp render_step({:validate_acceptance, field, opts}),
    do: "        .validate_acceptance(#{rust_string(field)}, #{rust_message(opts)})"

  defp render_constraint({:constraint, kind, fields, opts}) do
    "Constraint { kind: #{rust_string(kind)}.to_string(), fields: vec![#{Enum.map_join(fields, ", ", &"#{rust_string(&1)}.to_string()")}], " <>
      "name: #{rust_option(opts[:name])}, message: #{rust_option(opts[:message])} }"
  end

  defp rust_strings(values), do: Enum.map_join(values, ", ", &rust_string/1)
  defp rust_string(value), do: inspect(to_string(value))

  defp rust_message(opts) do
    case opts[:message] do
      nil -> "None"
      {message, _opts} -> "Some(#{rust_string(message)})"
      message -> "Some(#{rust_string(message)})"
    end
  end

  defp rust_option(nil), do: "None"
  defp rust_option(value), do: "Some(#{rust_string(value)}.to_string())"

  # The regex source with its flags inline, as the Rust `regex` crate
  # reads them. Ecto compiles with PCRE, so lookaround or backreferences
  # are written as they are and fail to compile on the Rust side, which
  # reports them
  defp rust_regex(%Regex{} = regex) do
    flags =
      regex
      |> Regex.opts()
      |> List.wrap()
      |> Enum.map_join(fn
        :caseless -> "i"
        :multiline -> "m"
        :dotall -> "s"
        :extended -> "x"
        flag when is_binary(flag) -> flag
        _other -> ""
      end)
      |> String.replace("u", "")

    pattern = if flags == "", do: Regex.source(regex), else: "(?#{flags})#{Regex.source(regex)}"

    hashes =
      String.duplicate(
        "#",
        1 + (Regex.scan(~r/"#+/, pattern) |> Enum.map(&String.length(hd(&1))) |> Enum.max(fn -> 0 end))
      )

    "r#{hashes}\"#{pattern}\"#{hashes}"
  end

  defp rust_number(%Decimal{} = decimal), do: rust_number(Decimal.to_float(decimal))
  defp rust_number(value) when is_integer(value), do: "#{value}.into()"
  defp rust_number(value) when is_float(value), do: "Number::from_f64(#{value}).unwrap_or_else(|| 0.into())"

  defp rust_json(value), do: Jason.encode!(value)
end
