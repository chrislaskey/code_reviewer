defmodule Core.Helpers.Json do
  @moduledoc "An Ecto type for a `jsonb` column that holds any JSON value, a string or null as well as an object."
  use Ecto.Type

  def type, do: :map
  def cast(value), do: {:ok, value}
  def load(value), do: {:ok, value}
  def dump(value), do: {:ok, value}
end
