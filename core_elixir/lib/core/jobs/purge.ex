defmodule Core.Jobs.Purge do
  @moduledoc """
  The daily purge, as `sync::purge` in the Rust core: deletes the change
  log older than the retention window, moves the feed's floor past it,
  and hard-deletes the file tombstones every device that pulled within
  the window has seen. Rows go children first, and only once their last
  change is at or below the floor.

  Only moving the floor holds the feed lock, briefly. The tombstones go
  after, a table at a time, without it, so no writer waits on them: a
  device that could still need one has a cursor below the floor, and
  resyncs. A run cut short leaves tombstones the next one removes.
  """
  use Oban.Worker, queue: :maintenance, max_attempts: 3

  alias Core.Data.{AppSyncChanges, AppSyncVersions}
  alias Core.Helpers.Time
  alias Core.Repo

  @retention_days 90

  @impl Oban.Worker
  def perform(_job) do
    {:ok, _} = run(Time.now())
    :ok
  end

  @doc "Purges as of `now`; returns the new floor and the rows removed per table."
  def run(now) do
    horizon = DateTime.add(now, -@retention_days, :day)

    {:ok, floor} =
      Repo.transaction(fn ->
        AppSyncVersions.lock()
        AppSyncChanges.delete_written_before(horizon)
      end)

    {:ok, %{floor: floor, removed: delete_tombstones(horizon, floor)}}
  end

  @doc "Deletes the tombstones before the horizon and at or below the floor, children first, a table at a time."
  def delete_tombstones(horizon, floor) do
    for table <- ~w(app_files_communities app_files), into: %{} do
      %{num_rows: count} =
        Repo.query!(
          "DELETE FROM #{table} t WHERE t.deleted_at IS NOT NULL AND t.deleted_at < $1 AND t.global_sync_version <= $2" <>
            if(table == "app_files",
              do: " AND NOT EXISTS (SELECT 1 FROM app_files_communities c WHERE c.file_id = t.id)",
              else: ""
            ),
          [horizon, floor]
        )

      {table, count}
    end
  end
end
