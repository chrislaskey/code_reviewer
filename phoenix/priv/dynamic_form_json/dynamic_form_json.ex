defmodule DynamicFormJson do
  @moduledoc """
  Writes the form definitions of functions marked
  `@universal_dynamic_form true` as `.dynamic_form.json` files next to
  them, for apps that render the same forms without the BEAM. See
  `priv/dynamic_form_json/README.md` and `mix help app.dynamic_forms`.
  """

  alias DynamicFormJson.Source

  @suffix ".dynamic_form.json"

  @doc """
  The marked forms of `app`'s modules, as `{forms, problems}`. The app must
  be compiled.
  """
  def forms(app) do
    _ = Application.load(app)

    {forms, problems} =
      app
      |> Application.spec(:modules)
      |> Enum.sort()
      |> Enum.filter(&Code.ensure_loaded?/1)
      |> Enum.map(&Source.forms/1)
      |> Enum.reduce({[], []}, fn {forms, errors}, {all, problems} ->
        {all ++ forms, problems ++ Enum.map(errors, &{:error, &1})}
      end)

    {forms, problems ++ collisions(forms)}
  end

  defp collisions(forms) do
    forms
    |> Enum.group_by(&path/1)
    |> Enum.filter(fn {_path, forms} -> length(forms) > 1 end)
    |> Enum.map(fn {path, [form | _] = forms} ->
      names = Enum.map_join(forms, ", ", &name/1)
      {:error, %{file: form.file, line: form.line, message: "#{names} would all write #{path}"}}
    end)
  end

  @doc "The file a form is written to: its function's name, next to its module."
  def path(%Source{file: file, function: function}),
    do: Path.join(Path.dirname(file), "#{function}#{@suffix}")

  @doc """
  The file's contents for a form: what its function returns, as JSON with
  the keys of each object sorted, so a file only changes when the form
  does. `{:error, error}` when it isn't a definition the library reads.
  """
  def render(%Source{module: module, function: function} = form) do
    definition = apply(module, function, [])

    with :ok <- json_only(definition),
         :ok <- parses(definition) do
      {:ok, definition |> sorted() |> Jason.encode!(pretty: true) |> Kernel.<>("\n")}
    else
      {:error, message} -> {:error, %{file: form.file, line: form.line, message: "#{name(form)}: #{message}"}}
    end
  end

  # The web app uses the function's value and the other apps the file, so
  # the two must be the same data
  defp json_only(definition) do
    if is_map(definition) and Jason.decode!(Jason.encode!(definition)) == definition,
      do: :ok,
      else: {:error, "returns JSON data only: a map with string keys, of strings, numbers, booleans, nil, lists and maps"}
  rescue
    Protocol.UndefinedError ->
      {:error, "returns JSON data only: a map with string keys, of strings, numbers, booleans, nil, lists and maps"}
  end

  defp parses(definition) do
    _ = DynamicForm.Parser.FromData.parse!(definition)
    :ok
  rescue
    error -> {:error, "dynamic_form can't read it: #{Exception.message(error)}"}
  end

  defp sorted(map) when is_map(map),
    do: map |> Enum.sort() |> Enum.map(fn {key, value} -> {key, sorted(value)} end) |> Jason.OrderedObject.new()

  defp sorted(list) when is_list(list), do: Enum.map(list, &sorted/1)
  defp sorted(value), do: value

  @doc """
  Writes every form's file, returning `{:written | :unchanged, path}` or
  `{:error, error}` for each.
  """
  def build(forms) do
    Enum.map(forms, fn form ->
      path = path(form)

      case render(form) do
        {:ok, contents} ->
          if File.read(path) == {:ok, contents} do
            {:unchanged, path}
          else
            File.write!(path, contents)
            {:written, path}
          end

        {:error, error} ->
          {:error, error}
      end
    end)
  end

  @doc """
  Checks every form's file without writing anything. Returns
  `{:missing | :stale, path}` or `{:error, error}` for each that doesn't
  match its function.
  """
  def check(forms) do
    Enum.flat_map(forms, fn form ->
      path = path(form)

      case {render(form), File.read(path)} do
        {{:error, error}, _file} -> [{:error, error}]
        {_contents, {:error, :enoent}} -> [{:missing, path}]
        {{:ok, contents}, {:ok, contents}} -> []
        _different -> [{:stale, path}]
      end
    end)
  end

  @doc """
  Returns `{:orphan, path}` for each file matching `pattern` whose function
  is no longer marked.
  """
  def orphans(forms, pattern \\ "lib/**/*#{@suffix}") do
    paths = MapSet.new(forms, &path/1)
    for path <- Path.wildcard(pattern), not MapSet.member?(paths, path), do: {:orphan, path}
  end

  @doc "Describes a result or problem for a person."
  def describe({:written, path}), do: "wrote #{path}"
  def describe({:unchanged, path}), do: "unchanged #{path}"
  def describe({:missing, path}), do: "#{path} is missing, run `mix app.dynamic_forms`"
  def describe({:stale, path}), do: "#{path} is out of date with its form, run `mix app.dynamic_forms`"

  def describe({:orphan, path}),
    do: "#{path} was written for a form no longer marked @universal_dynamic_form, delete it"

  def describe({:error, %{file: file, line: line, message: message}}), do: "#{file}:#{line}: #{message}"

  @doc "True for the results that need someone to act."
  def problem?({kind, _}), do: kind not in [:written, :unchanged]

  defp name(%Source{module: module, function: function}), do: "#{inspect(module)}.#{function}/0"
end
