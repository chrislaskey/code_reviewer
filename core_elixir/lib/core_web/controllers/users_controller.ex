defmodule CoreWeb.UsersController do
  use CoreWeb, :controller
  alias Core.App.Users

  def get_by_token(conn, params) do
    with {:ok, found} <- Users.get_by_token(params["token"]), do: json(conn, found)
  end

  def update(conn, params) do
    with {:ok, user} <- Users.update(params["actor_id"], params["name"]), do: json(conn, user)
  end
end
