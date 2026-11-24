defmodule App.WebWrites do
  @moduledoc """
  Writes the way the web makes them in the sync tests: one save is the
  messages a device would send, pushed at once through `App.Data.Sync`,
  and the server's rows and event log read back with plain SQL, so the
  tests don't depend on how the contexts are named.

      import App.WebWrites

      post_id = web_post(scope, [community.id])
      {%{"status" => "applied"}, link_id} = create(scope, "app_links", link(post_id))
      delete(scope, "app_posts", post_id)
      deleted_at("app_links", link_id)
  """

  alias App.Data.Memberships
  alias App.Data.Sync

  ## The web: one save is the messages a device would send, answered at once

  def web(scope, events, rows \\ []), do: wire(Sync.push(scope, rows, events))

  def create(scope, table, changes) do
    id = uuid()
    %{"events" => [answer]} = web(scope, [event(table, id, "create", 0, %{}, changes)])
    {answer, id}
  end

  # An edit from the row as the web loaded it, now
  def update(scope, table, id, changes) do
    loaded = row(table, id)
    base = Map.take(loaded, Map.keys(changes))
    %{"events" => [answer]} = web(scope, [event(table, id, "update", loaded["version"], base, changes)])
    answer
  end

  def delete(scope, table, id, at \\ DateTime.utc_now()),
    do: update(scope, table, id, %{"deleted_at" => DateTime.to_iso8601(at)})

  def undelete(scope, table, id), do: update(scope, table, id, %{"deleted_at" => nil})

  # A post by the scope's user in the communities: its create event and a
  # posts_communities row per community, in one push
  def web_post(scope, community_ids \\ []) do
    post_id = uuid()

    rows =
      for community_id <- community_ids do
        %{"table" => "app_posts_communities", "row" => community_row(scope, post_id, community_id)}
      end

    %{"events" => [%{"status" => "applied"}]} =
      web(scope, [event("app_posts", post_id, "create", 0, %{}, %{"body" => "a post"})], rows)

    post_id
  end

  def community_row(scope, post_id, community_id) do
    %{
      "id" => uuid(),
      "post_id" => post_id,
      "community_id" => community_id,
      "users_communities_id" => Memberships.membership(scope, community_id).id,
      "updated_at" => DateTime.to_iso8601(DateTime.utc_now())
    }
  end

  def link(post_id, label \\ "Docs"),
    do: %{"post_id" => post_id, "label" => label, "url" => "https://hexdocs.pm"}

  def tagging(post_id, tag_id, position \\ 0, note \\ nil),
    do: %{"post_id" => post_id, "tag_id" => tag_id, "position" => position, "note" => note}

  def tag(scope, name) do
    {%{"status" => "applied"}, id} = create(scope, "app_tags", %{"name" => name})
    id
  end

  def event(table, row_id, type, base_version, base, changes) do
    %{
      "id" => uuid(),
      "table" => table,
      "row_id" => row_id,
      "type" => type,
      "base_version" => base_version,
      "base" => base,
      "changes" => changes
    }
  end

  def uuid, do: Ecto.UUID.generate(version: 7)

  # As JSON would carry it
  def wire(term), do: term |> Jason.encode!() |> Jason.decode!()

  ## The server's rows and the event log, as stored

  def row(table, id) do
    case App.Repo.query!("SELECT * FROM #{table} WHERE id = ?", [id]) do
      %{rows: [values], columns: columns} -> columns |> Enum.zip(values) |> Map.new()
      %{rows: []} -> nil
    end
  end

  def deleted_at(table, id), do: row(table, id)["deleted_at"]

  def community_row_of(post_id) do
    %{rows: [[id]]} = App.Repo.query!("SELECT id FROM app_posts_communities WHERE post_id = ?", [post_id])
    id
  end

  def community_row_of(post_id, community_id) do
    %{rows: [[id]]} =
      App.Repo.query!("SELECT id FROM app_posts_communities WHERE post_id = ? AND community_id = ?", [
        post_id,
        community_id
      ])

    id
  end

  # The id of the user's membership row in the community, left or not
  def membership_id(scope, community) do
    %{rows: [[id]]} =
      App.Repo.query!("SELECT id FROM app_users_communities WHERE user_id = ? AND community_id = ?", [
        scope.user.id,
        community.id
      ])

    id
  end

  # A stream's events in order, with their maps decoded
  def stream(table, id) do
    %{rows: rows, columns: columns} =
      App.Repo.query!(
        "SELECT * FROM app_events WHERE stream_table = ? AND stream_id = ? ORDER BY sequence",
        [table, id]
      )

    for values <- rows do
      columns
      |> Enum.zip(values)
      |> Map.new()
      |> Map.update!("changes", &Jason.decode!/1)
      |> Map.update!("conflicts", &Jason.decode!/1)
    end
  end
end
