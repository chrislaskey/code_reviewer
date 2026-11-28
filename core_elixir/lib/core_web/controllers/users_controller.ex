defmodule CoreWeb.UsersController do
  use CoreWeb, :controller
  alias Core.App.Users

  def get_by_token(conn, params) do
    with {:ok, found} <- Users.get_by_token(params["token"]), do: json(conn, found)
  end

  def avatar_update(conn, params) do
    with {:ok, user} <- Users.avatar_update(params["actor_id"], params["avatar_url"]), do: json(conn, user)
  end

  def avatar_urls_list(conn, _params) do
    with {:ok, urls} <- Users.avatar_urls_list(), do: json(conn, urls)
  end

  def update(conn, params) do
    with {:ok, user} <- Users.update(params["actor_id"], params["name"]), do: json(conn, user)
  end
end
