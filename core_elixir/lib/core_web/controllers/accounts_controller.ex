defmodule CoreWeb.AccountsController do
  use CoreWeb, :controller
  alias Core.Accounts

  def sign_in(conn, params) do
    with {:ok, sent} <- Accounts.sign_in(params["email"], params["sign_up_enabled"] == true, params["locale"] || "en"),
         do: json(conn, sent)
  end

  def sign_in_with_code(conn, params) do
    with {:ok, matched} <- Accounts.sign_in_with_code(params["pending"], params["code"]), do: json(conn, matched)
  end

  def sign_in_confirm(conn, params) do
    with {:ok, session} <- Accounts.sign_in_confirm(params["pending"]), do: json(conn, session)
  end

  def sign_out(conn, params) do
    :ok = Accounts.sign_out(params["token"])
    json(conn, %{})
  end
end
