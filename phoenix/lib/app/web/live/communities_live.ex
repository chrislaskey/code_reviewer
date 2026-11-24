defmodule App.Web.CommunitiesLive do
  @moduledoc """
  Every community: create one, join and leave.
  """

  use App.Web, :live_view

  alias App.Data.Communities
  alias App.Data.Community
  alias App.Data.Memberships
  alias App.Data.UsersCommunities

  @impl true
  def mount(_params, _session, socket) do
    scope = socket.assigns.current_scope

    {:ok,
     socket
     |> assign(page_title: "Communities", form: new_form())
     |> stream(:communities, rows(scope))}
  end

  @impl true
  def render(assigns) do
    ~H"""
    <Layouts.app flash={@flash} current_scope={@current_scope}>
      <div id="communities-page" class="space-y-8">
        <.header>
          Communities
          <:subtitle>Create one, join and leave</:subtitle>
        </.header>

        <.form for={@form} id="community-form" phx-submit="create" class="flex items-end gap-2">
          <.input field={@form[:name]} label="New community" placeholder="Name" />
          <.button variant="primary" phx-disable-with="Creating...">Create</.button>
        </.form>

        <ul id="communities" phx-update="stream" class="divide-y divide-base-200">
          <li :for={{id, row} <- @streams.communities} id={id} class="flex items-center justify-between py-3">
            <.link
              :if={row.member?}
              navigate={~p"/communities/#{row.community}"}
              class="font-medium hover:underline"
            >
              {row.community.name}
            </.link>
            <span :if={!row.member?} class="font-medium">{row.community.name}</span>

            <.button :if={row.member?} phx-click="leave" phx-value-id={row.community.id}>
              Leave
            </.button>
            <.button :if={!row.member?} variant="primary" phx-click="join" phx-value-id={row.community.id}>
              Join
            </.button>
          </li>
        </ul>
      </div>
    </Layouts.app>
    """
  end

  @impl true
  def handle_event("create", %{"community" => params}, socket) do
    scope = socket.assigns.current_scope

    case Communities.create(params) do
      {:ok, community} ->
        {:ok, _membership} = Memberships.join(scope, community.id)

        {:noreply,
         socket
         |> assign(form: new_form())
         |> stream_insert(:communities, row(community, true))}

      {:error, changeset} ->
        {:noreply, assign(socket, form: to_form(changeset))}
    end
  end

  def handle_event("join", %{"id" => id}, socket) do
    scope = socket.assigns.current_scope

    with %Community{} = community <- Communities.get(id),
         {:ok, _membership} <- Memberships.join(scope, community.id) do
      {:noreply, stream_insert(socket, :communities, row(community, true))}
    else
      _ -> {:noreply, put_flash(socket, :error, "That community is gone.")}
    end
  end

  def handle_event("leave", %{"id" => id}, socket) do
    scope = socket.assigns.current_scope

    with %Community{} = community <- Communities.get(id),
         {:ok, _membership} <- Memberships.leave(scope, community.id) do
      {:noreply, stream_insert(socket, :communities, row(community, false))}
    else
      _ -> {:noreply, put_flash(socket, :error, "You are not a member of that community.")}
    end
  end

  defp new_form, do: to_form(Communities.change(%Community{}))

  defp rows(scope) do
    member_of =
      [filters: [user_id: scope.user.id]]
      |> UsersCommunities.list()
      |> MapSet.new(& &1.community_id)

    for community <- Communities.list() do
      row(community, MapSet.member?(member_of, community.id))
    end
  end

  defp row(community, member?), do: %{id: community.id, community: community, member?: member?}
end
