defmodule Core.Sync.Answers do
  @moduledoc "Answers to pushed events, as `sync::events::Answer` in the Rust core."

  @doc "A row on the wire: its table and the row."
  def table_row(_table, nil), do: nil
  def table_row(table, row), do: %{"table" => table, "row" => row}

  @doc "An applied event, a conflict when some fields did not apply, with the row as the server holds it."
  def applied(id, version, conflicts, server) do
    %{
      "id" => id,
      "status" => if(conflicts == %{}, do: "applied", else: "conflict"),
      "version" => version,
      "conflicts" => conflicts,
      "reason" => nil,
      "server" => server
    }
  end

  @doc "A rejected event, with the server's row when the device should take it."
  def rejected(id, reason, server \\ nil) do
    %{"id" => id, "status" => "rejected", "version" => nil, "conflicts" => %{}, "reason" => reason, "server" => server}
  end

  @doc "The answer recorded for a retried event, with the row as it stands now."
  def from_record(record, server) do
    %{
      "id" => record.id,
      "status" => record.status,
      "version" => record.version,
      "conflicts" => record.conflicts || %{},
      "reason" => nil,
      "server" => server
    }
  end

  @doc """
  The pushed event a command answers, as dispatch metadata: the
  projector records it with its answer in `app_events`.
  """
  def request(event) do
    Map.take(event, ~w(table row_id type base_version base changes made_at))
  end

  @doc "Field errors as the Rust `FieldErrors` prints them."
  def invalid(fields) do
    "invalid: " <>
      (fields
       |> Enum.sort()
       |> Enum.flat_map(fn {field, messages} -> Enum.map(messages, &"#{field}: #{&1}") end)
       |> Enum.join("; "))
  end
end
