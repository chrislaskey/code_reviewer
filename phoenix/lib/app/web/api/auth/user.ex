defmodule App.Web.Api.Auth.User do
  @moduledoc """
  The `auth.user` requests: the user a client is connected as, for a
  client that works offline and needs to know whose device it is.

    * `auth.user.get` - the user, as `id` and `email`.
  """

  def handle("get", _params, socket) do
    user = socket.assigns.current_scope.user
    {:reply, {:ok, %{id: user.id, email: user.email}}, socket}
  end

  def handle(_action, _params, _socket), do: :unknown
end
