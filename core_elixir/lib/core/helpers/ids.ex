defmodule Core.Helpers.Ids do
  @moduledoc "New ids, UUIDv7 as the Rust server makes them."

  @doc "A new id."
  def new, do: Ecto.UUID.generate(version: 7)

  @doc "The id, when it is a UUID; else nil."
  def cast(id) do
    case Ecto.UUID.cast(id) do
      {:ok, id} -> id
      :error -> nil
    end
  end
end
