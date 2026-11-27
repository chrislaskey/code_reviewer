defmodule Core.Data.AppSyncVersions do
  @moduledoc """
  The feed: one row of `app_sync_versions`, with the feed's id, the last
  global sync version given out, and the lowest cursor a pull still
  answers. Callers own the transaction.
  """
  alias Core.Repo

  @doc "The feed: `%{id, head, floor}`."
  def get do
    %{rows: [[id, head, floor]]} =
      Repo.query!(
        "SELECT feed_id, global_sync_version, minimum_global_sync_version FROM app_sync_versions WHERE id = 1"
      )

    %{id: Ecto.UUID.load!(id), head: head, floor: floor}
  end

  @doc """
  Gives out the next global sync version. The feed's row stays locked
  until the transaction ends, so versions commit in the order given.
  """
  def next do
    %{rows: [[version]]} =
      Repo.query!(
        "UPDATE app_sync_versions SET global_sync_version = global_sync_version + 1 WHERE id = 1 RETURNING global_sync_version"
      )

    version
  end

  @doc "Locks the feed before a transaction decides a change to synced rows."
  def lock do
    Repo.query!("SELECT id FROM app_sync_versions WHERE id = 1 FOR UPDATE")
    :ok
  end

  @doc """
  Gives the feed a new id, after a backup is restored: every device's
  cursor is then of another feed, so its next pull resyncs. A restored
  database may give out again versions a device already saw.
  """
  def renew_feed_id do
    Repo.query!("UPDATE app_sync_versions SET feed_id = gen_random_uuid() WHERE id = 1")
    :ok
  end

  @doc "Sets the lowest cursor a pull still answers."
  def update_floor(floor) do
    Repo.query!("UPDATE app_sync_versions SET minimum_global_sync_version = $1 WHERE id = 1", [floor])
    :ok
  end
end
