defmodule Core.Data.SyncedRow do
  @moduledoc """
  The sync columns every synced row carries, see `docs/design/offline-sync.md`,
  and the row as the wire carries it: the Rust struct as `serde` writes it.
  """
  alias Core.Data.AppSyncChanges
  alias Core.Helpers.Time
  alias Core.Repo

  @doc "Declares the sync columns in a schema."
  defmacro sync_columns do
    quote do
      field :inserted_at, :utc_datetime_usec
      field :updated_at, :utc_datetime_usec
      field :version, :integer, default: 0
      field :last_synced_at, :utc_datetime_usec
      field :global_sync_version, :integer
      field :deleted_at, :utc_datetime_usec
      field :deleted_by_id, Ecto.UUID
    end
  end

  @doc "The sync columns of a row, for the wire."
  def wire(row) do
    %{
      "id" => row.id,
      "inserted_at" => Time.to_wire(row.inserted_at),
      "updated_at" => Time.to_wire(row.updated_at),
      "version" => row.version,
      "last_synced_at" => Time.to_wire(row.last_synced_at),
      "global_sync_version" => row.global_sync_version && Integer.to_string(row.global_sync_version),
      "deleted_at" => Time.to_wire(row.deleted_at),
      "deleted_by_id" => row.deleted_by_id
    }
  end

  @doc """
  Writes a row, new or over its stored one, and publishes it: the row
  gets the next global sync version, recorded in the change log in the
  caller's transaction. Returns the row as written.
  """
  def write(row, table) do
    row = %{row | global_sync_version: AppSyncChanges.record(table, row.id)}
    Repo.insert!(row, on_conflict: :replace_all, conflict_target: :id)
  end

  @doc "A row changed by the server now: one version on, written now."
  def changed(row, now) do
    %{row | version: row.version + 1, updated_at: now, last_synced_at: now}
  end
end
