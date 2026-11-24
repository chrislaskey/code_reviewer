defmodule App.Data.Purge do
  @moduledoc """
  The purge job: hard-deletes what was soft-deleted more than the
  retention window ago, see "Leak 3" in `docs/design/offline-sync.md`.

  A synced row goes once it has been a tombstone for the whole window:
  `deleted_at` and `last_synced_at` both older than the horizon, so every
  client that pulled in the window has seen it. Children go before
  parents, and users after every `app_` table. The run then records the
  horizon in `app_purges`; `App.Data.Sync` tells a client whose cursor is
  older to resync from nothing, since it may have missed a tombstone.

  A deleted row's children were deleted with it, with the same
  `deleted_at`, see `App.Data.Deletes`, so they age out together. A
  purged parent still takes any child left through the foreign keys,
  such as a deleted user's posts.

  When the row a stream of `App.Data.EventLog` belongs to is gone, the
  stream goes too. A link purged on its own leaves its events in its
  post's stream.

  Run it from `mix app.purge` or `App.Release.purge/0`, on a schedule,
  such as a daily cron job on the server.
  """

  import Ecto.Query, only: [from: 2]

  alias App.Data.AppUsers
  alias App.Data.Comment
  alias App.Data.Comments
  alias App.Data.Communities
  alias App.Data.EventLog
  alias App.Data.Links
  alias App.Data.Post
  alias App.Data.Posts
  alias App.Data.PostsCommunities
  alias App.Data.PostsTags
  alias App.Data.Tag
  alias App.Data.Tags
  alias App.Data.UserProfiles
  alias App.Data.Users
  alias App.Data.UsersCommunities

  @retention_days 90

  # Children first
  @synced_contexts [
    Comments,
    PostsTags,
    Links,
    PostsCommunities,
    Posts,
    Tags,
    UsersCommunities,
    Communities,
    UserProfiles,
    AppUsers
  ]

  # The streams of the event log, by the table of the row they belong to
  @streams [{"app_posts", Post}, {"app_tags", Tag}, {"app_comments", Comment}]

  def retention_days, do: @retention_days

  @doc """
  Purges every table and records the horizon. Returns the horizon and how
  many rows went per table. `now:` is for tests.
  """
  def run(opts \\ []) do
    now = Keyword.get(opts, :now, DateTime.utc_now())
    horizon = DateTime.add(now, -@retention_days, :day)

    {:ok, counts} = App.Repo.transact(fn -> {:ok, purge_tables(horizon)} end)

    %{horizon: horizon, counts: counts}
  end

  defp purge_tables(horizon) do
    synced =
      for context <- @synced_contexts, into: %{} do
        {count, _} =
          context.delete_many(
            filters: [deleted_at_before: horizon, last_synced_at_before: horizon],
            hard: true
          )

        {context.schema().__schema__(:source), count}
      end

    events = EventLog.purge_streams(@streams)

    # Tokens go with the user through the foreign key
    {users, _} = Users.delete_many(filters: [deleted_at_before: horizon], hard: true)

    App.Repo.insert_all("app_purges", [
      %{id: Ecto.UUID.generate(), horizon: horizon, inserted_at: DateTime.utc_now()}
    ])

    synced |> Map.put("users", users) |> Map.put("app_events", events)
  end

  @doc """
  The horizon of the last purge: the oldest `last_synced_at` a client's
  cursor may have without a resync, or nil before the first purge.
  """
  def horizon do
    App.Repo.one(
      from(p in "app_purges",
        order_by: [desc: p.inserted_at],
        limit: 1,
        select: type(p.horizon, :utc_datetime_usec)
      )
    )
  end
end
