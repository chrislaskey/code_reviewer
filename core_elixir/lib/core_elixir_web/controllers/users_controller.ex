defmodule CoreElixirWeb.UsersController do
  use CoreElixirWeb, :controller
  alias CoreElixir.App.Users

  def get_by_token(conn, params) do
    with {:ok, found} <- Users.get_by_token(params["token"]), do: json(conn, found)
  end
end
