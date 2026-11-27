defmodule CoreElixir.Helpers.Time do
  @moduledoc "The server's clock, in microseconds as the database keeps it."

  @doc "Now, in UTC."
  def now, do: DateTime.utc_now(:microsecond)

  @doc "A time as the wire carries it: RFC 3339 in UTC, or nil."
  def to_wire(nil), do: nil
  def to_wire(%DateTime{} = time), do: DateTime.to_iso8601(time)

  @doc "A time from the wire, or nil."
  def from_wire(nil), do: {:ok, nil}

  def from_wire(text) when is_binary(text) do
    case DateTime.from_iso8601(text) do
      {:ok, time, _offset} -> {:ok, DateTime.truncate(time, :microsecond)}
      {:error, _} -> :error
    end
  end

  def from_wire(_), do: :error
end
