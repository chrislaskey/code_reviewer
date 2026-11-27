defmodule CoreElixir.Helpers.Ids do
  @moduledoc "New ids, UUIDv7 as the Rust server makes them."

  @doc "A new id."
  def new, do: Ecto.UUID.generate(version: 7)
end
