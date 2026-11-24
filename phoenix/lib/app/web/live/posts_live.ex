defmodule App.Web.PostsLive do
  @moduledoc """
  Everything the user can read across their communities, newest first,
  with each post's links and tags, and a form to write a post into the
  communities they choose. On their own posts the user adds and removes
  links and tags, and deletes the post.

  Every write goes through the events a device would push, applied at
  once, see `App.Data.Sync.save/4`: an error shows straight away.

  The form is `post_form/0`, rendered by `dynamic_form`, which validates
  it and sends `{:dynamic_form, :success, payload}` once it is valid. It
  is marked `@universal_dynamic_form true`, so the desktop app shows the
  same form from `post_form.dynamic_form.json`. The component's id changes after each post, which mounts
  a fresh, empty form.
  """

  use App.Web, :live_view

  alias App.Data.Link
  alias App.Data.Links
  alias App.Data.Post
  alias App.Data.Posting
  alias App.Data.Posts
  alias App.Data.PostsTags
  alias App.Data.PostTag
  alias App.Data.Tagging
  alias App.Data.UsersCommunities
  alias App.Web.DynamicForms

  @impl true
  def mount(_params, _session, socket) do
    scope = socket.assigns.current_scope

    communities =
      [filters: [user_id: scope.user.id], preloads: [:community]]
      |> UsersCommunities.list()
      |> Enum.map(& &1.community)
      |> Enum.sort_by(& &1.name)

    {:ok,
     socket
     |> assign(page_title: "Posts", post_form: post_form(communities), posts_written: 0)
     |> stream(:posts, Posting.feed(scope))}
  end

  @doc """
  The form for writing a post. Its `community_ids` choices are the
  user's communities, filled in by `post_form/1`.
  """
  @universal_dynamic_form true
  def post_form do
    %{
      "id" => "post-form",
      "elements" => [
        %{
          "type" => "comment",
          "name" => "body",
          "title" => "Write a post",
          "isRequired" => true,
          "validators" => [%{"type" => "text", "maxLength" => 10_000}]
        },
        %{"type" => "checkbox", "name" => "community_ids", "title" => "In these communities", "choices" => []}
      ]
    }
  end

  @doc """
  The post form with the communities as the choices of `community_ids`.
  """
  def post_form(communities) do
    choices = for community <- communities, do: %{"value" => community.id, "text" => community.name}

    post_form()
    |> DynamicForms.put_choices("community_ids", choices)
    |> DynamicForm.Parser.FromData.parse!()
  end

  @impl true
  def render(assigns) do
    ~H"""
    <Layouts.app flash={@flash} current_scope={@current_scope}>
      <div id="posts-page" class="space-y-8">
        <.header>
          Posts
          <:subtitle>Everything you can read in your communities</:subtitle>
          <:actions>
            <.link navigate={~p"/communities"} class="link">Communities</.link>
            <.link navigate={~p"/tags"} class="link">Tags</.link>
          </:actions>
        </.header>

        <div id="post-form">
          <DynamicForm.form
            id={"post-form-#{@posts_written}"}
            instance={@post_form}
            components={App.Web.CoreComponents}
            form_name="post"
            submit_text="Post"
          />
        </div>

        <ul id="posts" phx-update="stream" class="divide-y divide-base-200">
          <li :for={{id, post} <- @streams.posts} id={id} class="space-y-2 py-3">
            <p class="text-sm text-base-content/60">{post.owner.email}</p>
            <p class="whitespace-pre-wrap">{post.body}</p>

            <ul :if={post.links != []} class="space-y-1 text-sm">
              <li :for={link <- post.links} id={"link-#{link.id}"} class="flex items-center gap-2">
                <a href={link.url} class="link" target="_blank" rel="noopener noreferrer">{link.label}</a>
                <.button
                  :if={post.owner_id == @current_scope.user.id}
                  type="button"
                  class="btn btn-ghost btn-xs"
                  phx-click="remove_link"
                  phx-value-id={link.id}
                  aria-label={"Remove the link #{link.label}"}
                >
                  ×
                </.button>
              </li>
            </ul>

            <div :if={post.posts_tags != []} class="flex flex-wrap gap-1">
              <span :for={row <- post.posts_tags} id={"post-tag-#{row.id}"} class="badge badge-soft gap-1">
                {row.tag.name}
                <.button
                  :if={post.owner_id == @current_scope.user.id}
                  type="button"
                  class="btn btn-ghost btn-xs px-1"
                  phx-click="untag"
                  phx-value-id={row.id}
                  aria-label={"Remove the tag #{row.tag.name}"}
                >
                  ×
                </.button>
              </span>
            </div>

            <div :if={post.owner_id == @current_scope.user.id} class="flex flex-wrap items-center gap-2">
              <form id={"link-form-#{post.id}"} phx-submit="add_link" class="flex items-start gap-2">
                <.input type="hidden" name="post_id" value={post.id} />
                <.input
                  id={"link-label-#{post.id}"}
                  name="label"
                  value=""
                  placeholder="Label"
                  aria-label="Label"
                  class="input input-sm w-32"
                />
                <.input
                  id={"link-url-#{post.id}"}
                  name="url"
                  value=""
                  placeholder="https://"
                  aria-label="URL"
                  class="input input-sm w-48"
                />
                <.button type="submit" class="btn btn-sm">Add link</.button>
              </form>
              <form id={"tag-form-#{post.id}"} phx-submit="tag" class="flex items-start gap-2">
                <.input type="hidden" name="post_id" value={post.id} />
                <.input
                  id={"tag-name-#{post.id}"}
                  name="name"
                  value=""
                  placeholder="Tag"
                  aria-label="Tag"
                  class="input input-sm w-32"
                />
                <.button type="submit" class="btn btn-sm">Add tag</.button>
              </form>
              <.button phx-click="delete" phx-value-id={post.id} data-confirm="Delete this post?">
                Delete
              </.button>
            </div>
          </li>
        </ul>
      </div>
    </Layouts.app>
    """
  end

  @impl true
  def handle_info({:dynamic_form, :success, %DynamicForm.Payload{data: data}}, socket) do
    scope = socket.assigns.current_scope

    case Posting.write(scope, %{body: data.body}, data.community_ids || []) do
      {:ok, post} ->
        {:noreply,
         socket
         |> update(:posts_written, &(&1 + 1))
         |> stream_insert(:posts, Posting.feed_post(post.id), at: 0)}

      {:error, %Ecto.Changeset{}} ->
        {:noreply, put_flash(socket, :error, "Your post could not be saved.")}

      {:error, :not_a_member} ->
        {:noreply, put_flash(socket, :error, "You can only post in communities you are a member of.")}
    end
  end

  @impl true
  def handle_event("delete", %{"id" => id}, socket) do
    scope = socket.assigns.current_scope

    with %Post{} = post <- Posts.get(id),
         {:ok, _post} <- Posting.delete(scope, post) do
      {:noreply, stream_delete(socket, :posts, post)}
    else
      _ -> {:noreply, put_flash(socket, :error, "That post is gone, or not yours.")}
    end
  end

  def handle_event("add_link", %{"post_id" => post_id} = params, socket) do
    socket.assigns.current_scope
    |> Posting.add_link(post_id, params)
    |> saved(post_id, socket)
  end

  def handle_event("remove_link", %{"id" => id}, socket) do
    case Links.get(id) do
      %Link{} = link ->
        socket.assigns.current_scope |> Posting.remove_link(link) |> saved(link.post_id, socket)

      nil ->
        {:noreply, socket}
    end
  end

  def handle_event("tag", %{"post_id" => post_id, "name" => name}, socket) do
    case Posts.get(post_id) do
      %Post{} = post -> socket.assigns.current_scope |> Tagging.tag_post(post, name) |> saved(post_id, socket)
      nil -> {:noreply, socket}
    end
  end

  def handle_event("untag", %{"id" => id}, socket) do
    case PostsTags.get(id) do
      %PostTag{} = row -> socket.assigns.current_scope |> Tagging.untag(row) |> saved(row.post_id, socket)
      nil -> {:noreply, socket}
    end
  end

  # The post as it now stands, or the error at once
  defp saved({:ok, _row}, post_id, socket),
    do: {:noreply, stream_insert(socket, :posts, Posting.feed_post(post_id))}

  defp saved({:conflict, _conflicts, _row}, post_id, socket) do
    {:noreply,
     socket
     |> put_flash(:error, "Someone changed this first; here is what they saved.")
     |> stream_insert(:posts, Posting.feed_post(post_id))}
  end

  defp saved({:error, %Ecto.Changeset{} = changeset}, _post_id, socket),
    do: {:noreply, put_flash(socket, :error, errors_sentence(changeset))}

  defp saved({:error, reason}, _post_id, socket),
    do: {:noreply, put_flash(socket, :error, "That could not be saved (#{reason}).")}
end
