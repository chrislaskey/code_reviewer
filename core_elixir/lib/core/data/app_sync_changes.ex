defmodule Core.Data.AppSyncChanges do
  @moduledoc """
  The change log: one row of `app_sync_changes` per published change, by
  its global sync version, naming the table and row it changed. It holds
  no row data. Callers own the transaction.
  """
  alias Core.Data.AppSyncVersions
  alias Core.Helpers.Time
  alias Core.Repo

  @doc """
  Publishes a change to a row: gives out the next global sync version
  and records the change under it. Returns the version to store on the
  row, in the same transaction.
  """
  def record(table, row_id) do
    version = AppSyncVersions.next()

    Repo.query!(
      "INSERT INTO app_sync_changes (global_sync_version, table_name, row_id, written_at) VALUES ($1, $2, $3, $4)",
      [version, table, Ecto.UUID.dump!(row_id), Time.now()]
    )

    version
  end

  @doc """
  Deletes the oldest changes, up to the first one written at or after
  `horizon`, and moves the feed's floor to the last one deleted. Returns
  the floor.
  """
  def delete_written_before(horizon) do
    feed = AppSyncVersions.get()

    %{rows: [[first_kept]]} =
      Repo.query!("SELECT MIN(global_sync_version) FROM app_sync_changes WHERE written_at >= $1", [horizon])

    floor = if first_kept, do: first_kept - 1, else: feed.head
    Repo.query!("DELETE FROM app_sync_changes WHERE global_sync_version <= $1", [floor])
    AppSyncVersions.update_floor(floor)
    floor
  end
end
