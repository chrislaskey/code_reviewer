defmodule App.Data.Posting do
  @moduledoc """
  Writing a post and its links, adding it to and removing it from
  communities, deleting it, and reading a feed, over the `App.Data.Posts`,
  `App.Data.Links` and `App.Data.PostsCommunities` contexts.

  Posts and links change through events, applied by `App.Data.EventFlow`
  with the specs here. The web writes the way a device does: `write/3`
  and `delete/2` build the messages a device would push and run them
  through `App.Data.Sync`, see `docs/design/related-resources.md`.

  Only the owner adds their post to a community or removes it, and only
  to a community they are a member of. Adding a post back revives the row
  removed earlier. Deleting a post copies its `deleted_at` onto its links,
  its tags and its community rows, see `App.Data.Deletes`.

  Every change to a post, its links and its community rows is recorded in
  the post's stream of `App.Data.EventLog`.
  """

  import Ecto.Query, only: [from: 2]

  alias App.Data.Deletes
  alias App.Data.EventFlow
  alias App.Data.EventLog
  alias App.Data.Link
  alias App.Data.Links
  alias App.Data.Memberships
  alias App.Data.Post
  alias App.Data.PostCommunity
  alias App.Data.Posts
  alias App.Data.PostsCommunities
  alias App.Data.Sync
  alias App.Web.Scope

  @doc """
  Writes a post by the scope's user and adds it to the communities, in
  one transaction: the post's create event and a community row per
  community, as a device would push them. Returns `{:ok, post}`,
  `{:error, :not_a_member}` for a community the user is not a member of,
  and `{:error, changeset}` for a bad post.
  """
  def write(%Scope{} = scope, params, community_ids \\ []) do
    post_id = Ecto.UUID.generate(version: 7)
    body = params[:body] || params["body"]

    with {:ok, rows} <- community_rows(scope, post_id, community_ids) do
      event = Sync.event("app_posts", post_id, "create", 0, %{}, %{"body" => body})
      App.Repo.transact(fn -> run_all(scope, rows, event, post_id) end)
    end
  end

  # All or nothing: an error rolls the post back
  defp run_all(scope, rows, event, post_id) do
    with :ok <- Sync.outcome(Sync.run(scope, rows, [event])) do
      {:ok, Posts.get(post_id)}
    end
  end

  defp community_rows(scope, post_id, community_ids) do
    now = DateTime.to_iso8601(DateTime.utc_now())

    Enum.reduce_while(community_ids, {:ok, []}, fn community_id, {:ok, rows} ->
      case Memberships.membership(scope, community_id) do
        nil ->
          {:halt, {:error, :not_a_member}}

        membership ->
          row = %{
            "table" => "app_posts_communities",
            "row" => %{
              "id" => Ecto.UUID.generate(version: 7),
              "post_id" => post_id,
              "community_id" => community_id,
              "users_communities_id" => membership.id,
              "updated_at" => now
            }
          }

          {:cont, {:ok, rows ++ [row]}}
      end
    end)
  end

  @doc """
  Deletes the post, as an event from the post as loaded. Owner only.
  """
  def delete(%Scope{} = scope, %Post{} = post) do
    case Sync.save(scope, "app_posts", post, %{"deleted_at" => DateTime.utc_now()}) do
      {:ok, deleted} -> {:ok, deleted}
      {:conflict, _conflicts, row} -> {:ok, row}
      {:error, "not_owner"} -> {:error, :not_owner}
      error -> error
    end
  end

  @doc """
  Adds the post to the community, or adds it back. Owner only.
  """
  def add_to_community(
        %Scope{user: %{id: owner_id} = user} = scope,
        %Post{owner_id: owner_id} = post,
        community_id
      ) do
    case Memberships.membership(scope, community_id) do
      nil ->
        {:error, :not_a_member}

      membership ->
        case PostsCommunities.get_by([post_id: post.id, community_id: community_id],
               include_deleted: true
             ) do
          nil ->
            add(post, community_id, membership, user)

          %PostCommunity{deleted_at: nil} = row ->
            {:ok, row}

          %PostCommunity{} = row ->
            add_back(row, membership, user)
        end
    end
  end

  def add_to_community(_scope, _post, _community_id), do: {:error, :not_owner}

  defp add(post, community_id, membership, user) do
    params = %{post_id: post.id, community_id: community_id, users_communities_id: membership.id}

    with {:ok, row} <-
           %PostCommunity{} |> PostsCommunities.change(params) |> Deletes.inherit() |> App.Repo.insert(),
         {:ok, _event} <-
           EventLog.record(row, %{user_id: user.id, type: :create, base_version: 0, changes: params}) do
      {:ok, row}
    end
  end

  defp add_back(row, membership, user) do
    with {:ok, revived} <-
           PostsCommunities.update(row, %{users_communities_id: membership.id},
             changesets: [
               &PostCommunity.revive_changeset/3,
               fn changeset, _params, _options -> Deletes.inherit(changeset) end
             ]
           ),
         {:ok, _event} <-
           EventLog.record(revived, %{
             user_id: user.id,
             type: :update,
             base_version: row.version,
             changes: %{deleted_at: revived.deleted_at, users_communities_id: membership.id}
           }) do
      :ok = Deletes.copy_down(row, revived, user.id)
      {:ok, revived}
    end
  end

  @doc """
  Removes the post from the community. Owner only.
  """
  def remove_from_community(
        %Scope{user: %{id: owner_id} = user},
        %Post{owner_id: owner_id} = post,
        community_id
      ) do
    case PostsCommunities.get_by(post_id: post.id, community_id: community_id) do
      nil ->
        {:error, :not_found}

      row ->
        with {:ok, removed} <-
               PostsCommunities.delete(row,
                 deleted_by_id: user.id,
                 changesets: &PostCommunity.remove_changeset/3
               ),
             {:ok, _event} <-
               EventLog.record(removed, %{
                 user_id: user.id,
                 type: :update,
                 base_version: row.version,
                 changes: %{deleted_at: removed.deleted_at}
               }),
             :ok <- Deletes.copy_down(row, removed, user.id) do
          {:ok, removed}
        end
    end
  end

  def remove_from_community(_scope, _post, _community_id), do: {:error, :not_owner}

  @doc """
  Applies an event from a device or the web to a post or a link, checked
  for shape by `App.Data.Sync`. An event without a `:table` is a post's.
  See `App.Data.EventFlow` for what it returns.
  """
  def apply_event(%Scope{} = scope, event) do
    EventFlow.apply(scope, event, spec(Map.get(event, :table, "app_posts")))
  end

  defp spec("app_posts") do
    %{
      schema: Post,
      context: Posts,
      fields: %{create: ["body"], update: ["body", "deleted_at"]},
      unknown: "unknown_post",
      check_create: fn %Scope{user: user}, _changes -> {:ok, owner_id: user.id} end,
      check_update: fn
        %Scope{user: %{id: owner_id}}, %Post{owner_id: owner_id}, _changes -> :ok
        _scope, _post, _changes -> {:error, "not_owner"}
      end
    }
  end

  defp spec("app_links") do
    %{
      schema: Link,
      context: Links,
      fields: %{create: ["post_id", "label", "url"], update: ["label", "url", "deleted_at"]},
      unknown: "unknown_link",
      check_create: fn scope, changes ->
        with :ok <- own_post(scope, changes["post_id"]), do: {:ok, []}
      end,
      check_update: fn scope, link, _changes ->
        with {:error, reason, _row} <- own_post(scope, link.post_id), do: {:error, reason}
      end
    }
  end

  @doc """
  `:ok` when the post exists, deleted or not, and is the scope's user's;
  else `{:error, reason, nil}`.
  """
  def own_post(%Scope{user: user}, post_id) do
    case post_id && Ecto.UUID.cast(post_id) do
      {:ok, id} ->
        case Posts.get(id, include_deleted: true) do
          nil -> {:error, "unknown_post", nil}
          %Post{owner_id: owner_id} when owner_id == user.id -> :ok
          %Post{} -> {:error, "not_owner", nil}
        end

      _not_an_id ->
        {:error, "unknown_post", nil}
    end
  end

  @doc """
  Adds a link to the post, as the web does: one event, applied at once.
  Returns what `App.Data.Sync.save/4` does.
  """
  def add_link(%Scope{} = scope, post_id, params) do
    Sync.save(scope, "app_links", nil, %{
      "post_id" => post_id,
      "label" => params["label"],
      "url" => params["url"]
    })
  end

  @doc """
  Removes the link from its post.
  """
  def remove_link(%Scope{} = scope, %Link{} = link) do
    Sync.save(scope, "app_links", link, %{"deleted_at" => DateTime.utc_now()})
  end

  @doc """
  The posts the scope reads, newest first, with their owners, links and
  tags: inside a community, the community's feed; otherwise everything
  visible to the user across communities.
  """
  def feed(%Scope{community: %{id: community_id}}) do
    Posts.list(filters: [in_community: community_id], preloads: preloads())
  end

  def feed(%Scope{} = scope) do
    Posts.list(filters: [visible_to: scope], preloads: preloads())
  end

  @doc """
  The post as a feed shows it, or nil.
  """
  def feed_post(post_id), do: Posts.get(post_id, preloads: preloads())

  # A post's owner, live links, and live tags in the order it shows them
  defp preloads do
    links = from(l in Link, where: is_nil(l.deleted_at), order_by: [l.inserted_at, l.id])

    posts_tags =
      from(pt in App.Data.PostTag,
        join: tag in assoc(pt, :tag),
        where: is_nil(pt.deleted_at),
        order_by: [pt.position, fragment("lower(?)", tag.name)],
        preload: [tag: tag]
      )

    [:owner, links: links, posts_tags: posts_tags]
  end

  @doc """
  The live community rows of the post.
  """
  def communities_of(_scope, %Post{id: post_id}) do
    PostsCommunities.list(filters: [post_id: post_id])
  end
end
