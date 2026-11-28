defmodule CoreWeb.PasswordController do
  use CoreWeb, :controller
  alias Core.Accounts.Password

  def get(conn, params) do
    with {:ok, status} <- Password.get(params["user_id"]), do: json(conn, status)
  end

  def update(conn, params) do
    with {:ok, updated} <- Password.update(params["user_id"], params["password_hash"], params["locale"] || "en"),
         do: json(conn, updated)
  end

  def get_hash(conn, params) do
    with {:ok, hash} <- Password.get_hash(params["email"]), do: json(conn, hash)
  end
end
