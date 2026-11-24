defmodule App.Web.ProfileLive do
  @moduledoc """
  The user's own profile. A save sends only the fields changed since the
  form loaded; a field someone else changed first keeps their value and
  is listed with the user's, to reapply or let go.
  """

  use App.Web, :live_view

  alias App.Data.Profiles
  alias App.Data.UserProfiles

  @labels %{"display_name" => "Display name", "bio" => "Bio"}

  @impl true
  def mount(_params, _session, socket) do
    {:ok,
     socket
     |> assign(page_title: "Profile", conflicts: %{})
     |> load(Profiles.get(socket.assigns.current_scope))}
  end

  @impl true
  def render(assigns) do
    ~H"""
    <Layouts.app flash={@flash} current_scope={@current_scope}>
      <div id="profile-page" class="space-y-8">
        <.header>
          Profile
          <:subtitle>Your public details</:subtitle>
        </.header>

        <div
          :if={@conflicts != %{}}
          id="profile-conflicts"
          class="space-y-4 rounded-box border border-warning p-4"
        >
          <p class="font-medium">
            {map_size(@conflicts)} {if map_size(@conflicts) == 1, do: "change was", else: "changes were"} not saved: someone else changed the same thing first.
          </p>
          <div :for={{field, conflict} <- @conflicts} id={"conflict-#{field}"} class="space-y-1">
            <p class="font-medium">{@labels[field]}</p>
            <p>Your change: {shown(conflict["yours"])}</p>
            <p>Now: {shown(Map.fetch!(@loaded, String.to_existing_atom(field)))}</p>
            <div class="flex gap-2">
              <.button variant="primary" phx-click="reapply" phx-value-field={field}>Reapply</.button>
              <.button phx-click="let_go" phx-value-field={field}>Let it go</.button>
            </div>
          </div>
        </div>

        <.form for={@form} id="profile-form" phx-submit="save" class="space-y-4">
          <.input field={@form[:display_name]} label="Display name" />
          <.input field={@form[:bio]} type="textarea" label="Bio" />
          <.button variant="primary" phx-disable-with="Saving...">Save</.button>
        </.form>
      </div>
    </Layouts.app>
    """
  end

  @impl true
  def handle_event("save", %{"profile" => params}, socket) do
    save(socket, params, "Profile saved.")
  end

  def handle_event("reapply", %{"field" => field}, socket) do
    %{"yours" => yours} = socket.assigns.conflicts[field]

    socket
    |> assign(conflicts: Map.delete(socket.assigns.conflicts, field))
    |> save(%{field => yours}, "Reapplied.")
  end

  def handle_event("let_go", %{"field" => field}, socket) do
    {:noreply, assign(socket, conflicts: Map.delete(socket.assigns.conflicts, field))}
  end

  defp save(socket, params, saved) do
    case Profiles.save(socket.assigns.current_scope, socket.assigns.loaded, params) do
      {:ok, %{conflicts: conflicts}, profile} ->
        socket =
          socket
          |> assign(conflicts: Map.merge(socket.assigns.conflicts, conflicts))
          |> load(profile)

        {:noreply, if(conflicts == %{}, do: put_flash(socket, :info, saved), else: socket)}

      {:error, %Ecto.Changeset{} = changeset} ->
        {:noreply, assign(socket, form: to_form(changeset, as: :profile, action: :validate))}

      {:error, _reason, _profile} ->
        {:noreply, put_flash(socket, :error, "Your profile could not be saved.")}
    end
  end

  defp load(socket, profile) do
    assign(socket,
      loaded: profile,
      labels: @labels,
      form: to_form(UserProfiles.change(profile), as: :profile)
    )
  end

  defp shown(nil), do: "(empty)"
  defp shown(value), do: value
end
