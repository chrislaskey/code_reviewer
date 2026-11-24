defmodule App.Web.CommunityAuth do
  @moduledoc """
  Resolves the community of a LiveView inside one into the
  `current_scope`, after `App.Web.UserAuth` has resolved the user:

      live_session :require_authenticated_user,
        on_mount: [{App.Web.UserAuth, :require_authenticated}] do
        live "/communities/:id", CommunityLive
      end

      defmodule App.Web.CommunityLive do
        on_mount {App.Web.CommunityAuth, :require_community_member}
      end

  The community comes from the `:id` path param, and a user who is not a
  member is sent back to the list of communities.
  """

  use App.Web, :verified_routes

  alias App.Data.Communities
  alias App.Data.Memberships
  alias App.Web.Scope

  def on_mount(:require_community_member, %{"id" => id}, _session, socket) do
    scope = socket.assigns.current_scope

    with community when community != nil <- Communities.get(id),
         true <- Memberships.member?(scope, community.id) do
      scope = Scope.put_community(scope, community)
      {:cont, Phoenix.Component.assign(socket, :current_scope, scope)}
    else
      _ ->
        socket =
          socket
          |> Phoenix.LiveView.put_flash(:error, "You are not a member of this community.")
          |> Phoenix.LiveView.redirect(to: ~p"/communities")

        {:halt, socket}
    end
  end
end
