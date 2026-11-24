defmodule App.Web.TagsLive do
  @moduledoc """
  The user's tags, with how many posts carry each: make one, rename one,
  or delete one, which takes it off its posts and leaves the posts.

  Every write goes through the events a device would push, applied at
  once, see `App.Data.Sync.save/4`.
  """

  use App.Web, :live_view

  alias App.Data.Sync
  alias App.Data.Tag
  alias App.Data.Tagging
  alias App.Data.Tags

  @impl true
  def mount(_params, _session, socket) do
    {:ok, socket |> assign(page_title: "Tags") |> load()}
  end

  defp load(socket), do: assign(socket, tags: Tagging.tags_with_counts(socket.assigns.current_scope))

  @impl true
  def render(assigns) do
    ~H"""
    <Layouts.app flash={@flash} current_scope={@current_scope}>
      <div id="tags-page" class="space-y-8">
        <.header>
          Tags
          <:subtitle>Yours, for your own posts</:subtitle>
          <:actions>
            <.link navigate={~p"/posts"} class="link">Posts</.link>
          </:actions>
        </.header>

        <form id="new-tag-form" phx-submit="create" class="flex items-start gap-2">
          <.input id="new-tag-name" name="name" value="" placeholder="New tag" aria-label="New tag" />
          <.button type="submit" variant="primary">Make tag</.button>
        </form>

        <p :if={@tags == []} class="text-base-content/60">No tags yet.</p>

        <ul id="tags" class="divide-y divide-base-200">
          <li :for={{tag, count} <- @tags} id={"tag-#{tag.id}"} class="flex flex-wrap items-center gap-3 py-3">
            <span class="font-medium">{tag.name}</span>
            <span class="text-sm text-base-content/60">
              {ngettext("1 post", "%{count} posts", count)}
            </span>
            <form id={"rename-tag-#{tag.id}"} phx-submit="rename" class="flex items-start gap-2">
              <.input type="hidden" name="tag_id" value={tag.id} />
              <.input
                id={"tag-name-#{tag.id}"}
                name="name"
                value={tag.name}
                aria-label={"New name for #{tag.name}"}
                class="input input-sm"
              />
              <.button type="submit" class="btn btn-sm">Rename</.button>
            </form>
            <.button
              phx-click="delete"
              phx-value-id={tag.id}
              data-confirm={"Delete #{tag.name}? It comes off every post that has it."}
            >
              Delete
            </.button>
          </li>
        </ul>
      </div>
    </Layouts.app>
    """
  end

  @impl true
  def handle_event("create", %{"name" => name}, socket) do
    socket.assigns.current_scope |> Sync.save("app_tags", nil, %{"name" => name}) |> saved(socket)
  end

  def handle_event("rename", %{"tag_id" => id, "name" => name}, socket) do
    with_tag(socket, id, &Tagging.rename_tag(&1, &2, name))
  end

  def handle_event("delete", %{"id" => id}, socket) do
    with_tag(socket, id, &Tagging.delete_tag/2)
  end

  defp with_tag(socket, id, save) do
    case Tags.get(id) do
      %Tag{} = tag -> socket.assigns.current_scope |> save.(tag) |> saved(socket)
      nil -> {:noreply, load(socket)}
    end
  end

  defp saved({:ok, _tag}, socket), do: {:noreply, load(socket)}

  defp saved({:conflict, _conflicts, _tag}, socket) do
    {:noreply,
     socket |> put_flash(:error, "Someone changed this tag first; here is what they saved.") |> load()}
  end

  defp saved({:error, %Ecto.Changeset{} = changeset}, socket),
    do: {:noreply, put_flash(socket, :error, errors_sentence(changeset))}

  defp saved({:error, reason}, socket),
    do: {:noreply, put_flash(socket, :error, "That could not be saved (#{reason}).")}
end
