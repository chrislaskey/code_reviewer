defmodule EctoRust do
  @moduledoc """
  Writes the changeset functions marked `@universal_changeset true` as Rust,
  and the NIF that runs that Rust in this app. See `priv/rust/README.md`
  and `mix help app.rust`.

  Each schema module with a marked function gets `<schema>.rs` next to it,
  written from the functions' source by `EctoRust.Transpile`. While any
  function is marked, `EctoRust.Native` writes the NIF crate and the
  module that loads it; when none is, both are removed, and nothing needs
  the Rust toolchain.
  """

  alias EctoRust.{Native, Source, Transpile}

  @doc """
  The marked functions of `app`'s schema modules, grouped by module in
  `{module, sources}` pairs. The app must be compiled.
  """
  def marked(app) do
    _ = Application.load(app)

    app
    |> Application.spec(:modules)
    |> Enum.filter(&(Code.ensure_loaded?(&1) and function_exported?(&1, :__changeset__, 0)))
    |> Enum.sort()
    |> Enum.map(&{&1, Source.marked(&1)})
    |> Enum.reject(fn {_module, sources} -> sources == [] end)
  end

  @doc """
  Writes the `.rs` file of every module and the NIF, returning
  `{:written | :unchanged | :removed, path}` or `{:error, problems}` for each.
  """
  def build(marked) do
    results =
      Enum.map(marked, fn {_module, sources} -> write(rust_path(sources), Transpile.module(sources)) end)

    # The NIF crate includes every `.rs` by path, so it is only written
    # once each of them is
    if Enum.any?(results, &problem?/1),
      do: results,
      else: results ++ build_native(List.flatten(Keyword.values(marked)))
  end

  # Mix's compiler touches a source whose external resources changed or
  # whose module asks to be recompiled, which the loader's does through
  # Rustler, and `File.touch` recreates a removed file empty. Compiling
  # once after the removal makes the manifest forget the module, and the
  # empty file it leaves behind is removed again
  defp build_native([]) do
    removed =
      for path <- [Native.elixir_path(), Native.rust_path()], File.exists?(path) do
        File.rm!(path)
        {:removed, path}
      end

    if removed != [] do
      Mix.Task.rerun("compile.elixir", ["--no-protocol-consolidation"])
      for path <- [Native.elixir_path(), Native.rust_path()], File.exists?(path), do: File.rm!(path)
    end

    removed
  end

  # The crate first: the module compiles it, so the module must never
  # exist without it
  defp build_native(sources) do
    File.mkdir_p!(Path.dirname(Native.elixir_path()))
    [write(Native.rust_path(), Native.rust(sources)), write(Native.elixir_path(), Native.elixir(sources))]
  end

  @doc """
  Returns `{:missing | :stale | :orphan, path}` or `{:error, problems}` for
  each file that does not match its functions.
  """
  def check(marked) do
    Enum.map(marked, fn {_module, sources} -> compare(rust_path(sources), Transpile.module(sources)) end)
    |> Kernel.++(check_native(List.flatten(Keyword.values(marked))))
    |> Enum.reject(&is_nil/1)
  end

  defp check_native([]) do
    for path <- [Native.elixir_path(), Native.rust_path()], File.exists?(path), do: {:orphan, path}
  end

  defp check_native(sources) do
    [compare(Native.elixir_path(), Native.elixir(sources)), compare(Native.rust_path(), Native.rust(sources))]
  end

  defp write(path, {:ok, contents}) do
    if File.read(path) == {:ok, contents} do
      {:unchanged, path}
    else
      File.write!(path, contents)
      {:written, path}
    end
  end

  defp write(_path, {:error, problems}), do: {:error, problems}

  defp compare(path, {:ok, contents}) do
    case File.read(path) do
      {:ok, ^contents} -> nil
      {:ok, _other} -> {:stale, path}
      {:error, _} -> {:missing, path}
    end
  end

  defp compare(_path, {:error, problems}), do: {:error, problems}

  defp rust_path([%Source{file: file} | _]), do: Path.rootname(file) <> ".rs"

  def problem?({:error, _}), do: true
  def problem?({kind, _}), do: kind in [:missing, :stale, :orphan]

  def describe({:written, path}), do: "Wrote #{path}"
  def describe({:unchanged, path}), do: "Unchanged #{path}"
  def describe({:removed, path}), do: "Removed #{path}, no function is marked @universal_changeset true"

  def describe({:orphan, path}),
    do: "#{path} exists but no function is marked @universal_changeset true, run mix app.rust"

  def describe({:missing, path}), do: "#{path} is missing, run mix app.rust"
  def describe({:stale, path}), do: "#{path} is out of date, run mix app.rust"

  def describe({:error, problems}) when is_list(problems),
    do: Enum.map_join(problems, "\n", &describe({:error, &1}))

  def describe({:error, %{file: file, line: line, message: message}}), do: "#{file}:#{line}: #{message}"
end
