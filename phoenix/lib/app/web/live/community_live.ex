defmodule App.Web.CommunityLive do
  @moduledoc """
  One community and its feed, for its members, with each post's thread
  of comments here: any member comments, and deletes their own comment;
  a post's owner deletes any comment on it. The community comes from the
  path and the membership check from `App.Web.CommunityAuth`.

  Every comment goes through the events a device would push, applied at
  once, see `App.Data.Sync.save/4`.
  """

  use App.Web, :live_view

  alias App.Data.Comment
  alias App.Data.Commenting
  alias App.Data.Comments
  alias App.Data.Memberships
  alias App.Data.Posting
  alias App.Data.Posts
  alias App.Data.UsersCommunities

  on_mount {App.Web.CommunityAuth, :require_community_member}

  @impl true
  def mount(_params, _session, socket) do
    scope = socket.assigns.current_scope
    members = UsersCommunities.list(filters: [community_id: scope.community.id])

    {:ok,
     socket
     |> assign(page_title: scope.community.name, member_count: length(members))
     |> stream(:posts, items(scope, Posting.feed(scope)))}
  end

  # A post with its thread here, as the stream shows it
  defp items(%{community: community}, posts) do
    threads = Commenting.threads(community.id, Enum.map(posts, & &1.id))
    for post <- posts, do: %{id: post.id, post: post, comments: Map.get(threads, post.id, [])}
  end

  @impl true
  def render(assigns) do
    ~H"""
    <Layouts.app flash={@flash} current_scope={@current_scope}>
      <div id="community-page" class="space-y-8">
        <.header>
          {@current_scope.community.name}
          <:subtitle>
            A community with {@member_count} member(s)
          </:subtitle>
          <:actions>
            <.button id="leave-community" phx-click="leave" data-confirm="Leave this community?">
              Leave
            </.button>
          </:actions>
        </.header>

        <ul id="posts" phx-update="stream" class="divide-y divide-base-200">
          <li :for={{id, %{post: post} = item} <- @streams.posts} id={id} class="space-y-2 py-3">
            <p class="text-sm text-base-content/60">{post.owner.email}</p>
            <p class="whitespace-pre-wrap">{post.body}</p>
            <.button
              :if={post.owner_id == @current_scope.user.id}
              phx-click="remove"
              phx-value-id={post.id}
            >
              Remove from {@current_scope.community.name}
            </.button>

            <ul id={"comments-#{post.id}"} class="ml-4 space-y-1 border-l border-base-200 pl-3 text-sm">
              <li :for={comment <- item.comments} id={"comment-#{comment.id}"} class="flex items-start gap-2">
                <span class="text-base-content/60">{comment.owner.email}</span>
                <span class="whitespace-pre-wrap">{comment.body}</span>
                <.button
                  :if={Commenting.deletable?(@current_scope, comment, post)}
                  type="button"
                  class="btn btn-ghost btn-xs"
                  phx-click="delete_comment"
                  phx-value-id={comment.id}
                  aria-label="Delete this comment"
                >
                  ×
                </.button>
              </li>
            </ul>

            <form id={"comment-form-#{post.id}"} phx-submit="comment" class="ml-4 flex items-start gap-2">
              <.input type="hidden" name="post_id" value={post.id} />
              <.input
                id={"comment-body-#{post.id}"}
                name="body"
                value=""
                placeholder="Comment"
                aria-label="Comment"
                class="input input-sm w-72"
              />
              <.button type="submit" class="btn btn-sm">Comment</.button>
            </form>
          </li>
        </ul>

        <div class="flex gap-4">
          <.link navigate={~p"/posts"} class="link">Write a post</.link>
          <.link navigate={~p"/communities"} class="link">All communities</.link>
        </div>
      </div>
    </Layouts.app>
    """
  end

  @impl true
  def handle_event("remove", %{"id" => id}, socket) do
    scope = socket.assigns.current_scope

    with %App.Data.Post{} = post <- Posts.get(id),
         {:ok, _row} <- Posting.remove_from_community(scope, post, scope.community.id) do
      {:noreply, stream_delete(socket, :posts, post)}
    else
      _ -> {:noreply, put_flash(socket, :error, "That post is gone from here, or not yours.")}
    end
  end

  def handle_event("leave", _params, socket) do
    scope = socket.assigns.current_scope
    {:ok, _membership} = Memberships.leave(scope, scope.community.id)

    {:noreply,
     socket
     |> put_flash(:info, "You left #{scope.community.name}.")
     |> push_navigate(to: ~p"/communities")}
  end

  def handle_event("comment", %{"post_id" => post_id, "body" => body}, socket) do
    scope = socket.assigns.current_scope

    case Posts.get(post_id) do
      nil -> {:noreply, socket}
      post -> scope |> Commenting.comment(post, body) |> saved(post_id, socket)
    end
  end

  def handle_event("delete_comment", %{"id" => id}, socket) do
    case Comments.get(id, preloads: [:post_community]) do
      %Comment{} = comment ->
        socket.assigns.current_scope
        |> Commenting.delete(comment)
        |> saved(comment.post_community.post_id, socket)

      nil ->
        {:noreply, socket}
    end
  end

  # The post with its thread as it now stands, or the error at once
  defp saved({:error, %Ecto.Changeset{} = changeset}, _post_id, socket),
    do: {:noreply, put_flash(socket, :error, errors_sentence(changeset))}

  defp saved({:error, reason}, _post_id, socket),
    do: {:noreply, put_flash(socket, :error, "That could not be saved (#{reason}).")}

  defp saved(_saved, post_id, socket) do
    scope = socket.assigns.current_scope
    [item] = items(scope, [Posting.feed_post(post_id)])
    {:noreply, stream_insert(socket, :posts, item)}
  end
end
