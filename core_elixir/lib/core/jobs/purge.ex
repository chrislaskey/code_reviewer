defmodule Core.Jobs.Purge do
  @moduledoc """
  The daily purge, as `sync::purge` in the Rust core: deletes the change
  log older than the retention window, moves the feed's floor past it,
  and hard-deletes the file tombstones every device that pulled within
  the window has seen. Rows go children first, and only once their last
  change is at or below the floor.
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

    Repo.transaction(fn ->
      AppSyncVersions.lock()
      floor = AppSyncChanges.delete_written_before(horizon)

      removed =
        for table <- ~w(app_files_communities app_files), into: %{} do
          %{num_rows: count} =
            Repo.query!(
              "DELETE FROM #{table} t WHERE t.deleted_at IS NOT NULL AND t.deleted_at < $1 AND t.last_synced_at < $1 AND t.global_sync_version <= $2" <>
                if(table == "app_files",
                  do: " AND NOT EXISTS (SELECT 1 FROM app_files_communities c WHERE c.file_id = t.id)",
                  else: ""
                ),
              [horizon, floor]
            )

          {table, count}
        end

      %{floor: floor, removed: removed}
    end)
  end
end
