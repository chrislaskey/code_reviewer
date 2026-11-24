defmodule App.Data.Commenting do
  @moduledoc """
  Comments on a post in a community, over the `App.Data.Comments`
  context. They change through events, applied by `App.Data.EventFlow`
  with the spec here; the web writes them with `App.Data.Sync.save/4` as
  a device would. See `docs/design/comments.md`.

  Any live member of the community comments. The commenter edits their
  comment while they are a member; they or the post's owner delete it,
  and a commenter can't bring back what the post's owner deleted. A
  comment stays when its commenter leaves, and goes with its thread when
  the post leaves the community or is deleted, see `App.Data.Deletes`.

  Each comment's events are its own stream in `App.Data.EventLog`.
  """

  import Ecto.Query, only: [from: 2]

  alias App.Data.Comment
  alias App.Data.Comments
  alias App.Data.EventFlow
  alias App.Data.Memberships
  alias App.Data.Post
  alias App.Data.PostCommunity
  alias App.Data.Posts
  alias App.Data.PostsCommunities
  alias App.Data.Sync
  alias App.Web.Scope

  @doc """
  Applies an event from a device or the web to a comment, checked for
  shape by `App.Data.Sync`. See `App.Data.EventFlow` for what it returns.
  """
  def apply_event(%Scope{} = scope, event), do: EventFlow.apply(scope, event, spec())

  defp spec do
    %{
      schema: Comment,
      context: Comments,
      fields: %{create: ["posts_communities_id", "body"], update: ["body", "deleted_at"]},
      unknown: "unknown_comment",
      check_create: &check_create/2,
      check_update: &check_update/3
    }
  end

  defp check_create(%Scope{user: user} = scope, changes) do
    with {:ok, thread} <- thread(changes["posts_communities_id"]) do
      if Memberships.member?(scope, thread.community_id),
        do: {:ok, owner_id: user.id},
        else: {:error, "not_a_member", nil}
    end
  end

  defp check_update(%Scope{user: user} = scope, %Comment{} = comment, changes) do
    {:ok, thread} = thread(comment.posts_communities_id)
    post_owner_id = Posts.get(thread.post_id, include_deleted: true).owner_id
    commenter? = comment.owner_id == user.id
    post_owner? = post_owner_id == user.id

    cond do
      commenter? and not post_owner? and not Memberships.member?(scope, thread.community_id) ->
        {:error, "not_a_member"}

      may_change?(commenter?, post_owner?, changes, comment.deleted_by_id == post_owner_id) ->
        :ok

      true ->
        {:error, "not_owner"}
    end
  end

  # The commenter edits; they or the post's owner delete; and what the
  # post's owner deleted, only they bring back
  defp may_change?(commenter?, post_owner?, changes, deleted_by_post_owner?) do
    edits? = Map.has_key?(changes, "body")
    deletes? = Map.has_key?(changes, "deleted_at")
    undeletes? = deletes? and changes["deleted_at"] == nil

    (commenter? or not edits?) and (commenter? or post_owner? or not deletes?) and
      (post_owner? or not (undeletes? and deleted_by_post_owner?))
  end

  # The post in the community a comment is on, removed or not
  defp thread(id) do
    case id && Ecto.UUID.cast(id) do
      {:ok, id} ->
        case PostsCommunities.get(id, include_deleted: true) do
          nil -> {:error, "unknown_post_community", nil}
          thread -> {:ok, thread}
        end

      _not_an_id ->
        {:error, "unknown_post_community", nil}
    end
  end

  @doc """
  Comments on the post in the scope's community, as the web does: one
  event, applied at once. Returns what `App.Data.Sync.save/4` does, or
  `{:error, "unknown_post_community"}` when the post isn't there.
  """
  def comment(%Scope{community: %{id: community_id}} = scope, %Post{} = post, body) do
    case PostsCommunities.get_by(post_id: post.id, community_id: community_id) do
      nil -> {:error, "unknown_post_community"}
      thread -> Sync.save(scope, "app_comments", nil, %{"posts_communities_id" => thread.id, "body" => body})
    end
  end

  @doc """
  Deletes the comment, as its commenter or the post's owner.
  """
  def delete(%Scope{} = scope, %Comment{} = comment) do
    Sync.save(scope, "app_comments", comment, %{"deleted_at" => DateTime.utc_now()})
  end

  @doc """
  Whether the scope's user may delete the comment: theirs, or on their
  post. For the web to show the button; the event is checked anyway.
  """
  def deletable?(%Scope{user: user}, %Comment{} = comment, %Post{} = post),
    do: comment.owner_id == user.id or post.owner_id == user.id

  @doc """
  The live comments in the community on each of the posts, oldest first,
  with their commenters, as `%{post_id => [comment]}`.
  """
  def threads(community_id, post_ids) do
    App.Repo.all(
      from(c in Comment,
        join: pc in PostCommunity,
        on: pc.id == c.posts_communities_id,
        where: pc.community_id == ^community_id and pc.post_id in ^post_ids and is_nil(c.deleted_at),
        order_by: [c.inserted_at, c.id],
        preload: [:owner],
        select: {pc.post_id, c}
      )
    )
    |> Enum.group_by(&elem(&1, 0), &elem(&1, 1))
  end
end
