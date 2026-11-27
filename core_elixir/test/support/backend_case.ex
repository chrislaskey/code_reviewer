defmodule Core.BackendCase do
  @moduledoc """
  A test against the whole backend, from an empty database: each test
  stops the app, empties the event store and the tables, makes a new
  feed, and starts the app again. Commanded's processes keep their own
  connections, so a sandbox cannot isolate them.
  """
  use ExUnit.CaseTemplate

  using do
    quote do
      import Core.BackendCase
    end
  end

  setup do
    Application.stop(:core)
    config = Core.EventStore.config()
    {:ok, conn} = Postgrex.start_link(Keyword.drop(config, [:schema]))
    EventStore.Storage.Initializer.reset!(conn, config)

    Postgrex.query!(
      conn,
      "TRUNCATE accounts_tokens, accounts_users, app_users, app_communities, app_users_communities, app_files, app_files_communities, app_events, app_sync_changes, projection_versions, oban_jobs CASCADE",
      []
    )

    Postgrex.query!(
      conn,
      "UPDATE app_sync_versions SET feed_id = gen_random_uuid(), global_sync_version = 0, minimum_global_sync_version = 0",
      []
    )

    GenServer.stop(conn)
    {:ok, _} = Application.ensure_all_started(:core)
    :ok
  end

  @doc "A signed-up, confirmed user with a token: `%{user, email, token}`."
  def signed_up(email) do
    {:ok, %{"pending" => pending, "mail" => %{"code" => code}}} = Core.Accounts.sign_in(email, true, "en")
    {:ok, %{"pending" => confirm}} = Core.Accounts.sign_in_with_code(pending, code)
    {:ok, session} = Core.Accounts.sign_in_confirm(confirm, "2026-11-22")
    session
  end

  @doc "The user's personal community's id."
  def personal_community(user_id) do
    [id] = for %{"community" => %{"id" => id}, "membership" => %{}} <- Core.App.Communities.list(user_id), do: id
    id
  end

  @doc "A pushed file create, with its bytes kept on the row."
  def file_create(file_id, community_id, name) do
    %{
      "id" => Core.Helpers.Ids.new(),
      "table" => "app_files",
      "row_id" => file_id,
      "type" => "create",
      "base_version" => 0,
      "base" => %{},
      "changes" => %{
        "name" => name,
        "content_type" => "text/plain",
        "size" => 5,
        "sha256" => String.duplicate("a", 64),
        "content" => "hello",
        "communities" => [%{"id" => Core.Helpers.Ids.new(), "community_id" => community_id}]
      },
      "made_at" => Core.Helpers.Time.to_wire(Core.Helpers.Time.now())
    }
  end

  @doc "Every page of a pull from `cursor`, to the end: `{rows, last cursor}`."
  def pull_all(user_id, cursor, limit \\ 200) do
    pull_pages(user_id, %{"cursor" => cursor, "after" => nil, "limit" => limit}, [])
  end

  defp pull_pages(user_id, request, rows) do
    {:ok, page} = Core.Sync.pull(user_id, request)
    rows = rows ++ page["rows"]

    case page["next"] do
      nil -> {rows, page["cursor"]}
      next -> pull_pages(user_id, %{"cursor" => nil, "after" => next, "limit" => request["limit"]}, rows)
    end
  end

  @doc "The ids of the rows of a table."
  def ids_of(rows, table), do: for(%{"table" => ^table, "row" => row} <- rows, do: row["id"])
end
