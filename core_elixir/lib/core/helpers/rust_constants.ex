defmodule Core.Helpers.RustConstants do
  @moduledoc """
  A list of strings the Rust core declares as a constant, read from its
  source when the module that asks compiles, so both backends keep one
  list: `pub const NAME: &[&str] = &["a", "b"];`.
  """

  @doc "The strings of the constant `name` in the Rust file at `path`."
  def strings!(path, name) do
    source = File.read!(path)

    case Regex.run(~r/pub const #{name}: &\[&str\] = &\[(.*?)\];/s, source) do
      [_, items] -> for [_, item] <- Regex.scan(~r/"([^"]*)"/, items), do: item
      nil -> raise "no constant #{name} in #{path}"
    end
  end
end
