defmodule CoreWeb.UsersController do
  use CoreWeb, :controller
  alias Core.App.Users

  def get_by_token(conn, params) do
    with {:ok, found} <- Users.get_by_token(params["token"]), do: json(conn, found)
  end
end
