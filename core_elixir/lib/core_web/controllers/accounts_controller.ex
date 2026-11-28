defmodule CoreWeb.AccountsController do
  use CoreWeb, :controller
  alias Core.Accounts

  def users_get(conn, params) do
    with {:ok, account} <- Accounts.users_get(params["user_id"]), do: json(conn, account)
  end

  def sign_in(conn, params) do
    with {:ok, sent} <- Accounts.sign_in(params["email"], params["sign_up_enabled"] == true, params["locale"] || "en"),
         do: json(conn, sent)
  end

  def sign_in_from_trial(conn, params) do
    with {:ok, sent} <- Accounts.sign_in_from_trial(params["trial_id"], params["email"], params["locale"] || "en"),
         do: json(conn, sent)
  end

  def sign_in_with_code(conn, params) do
    with {:ok, matched} <- Accounts.sign_in_with_code(params["pending"], params["code"]), do: json(conn, matched)
  end

  def resend_code_to(conn, params) do
    with {:ok, sent_to} <- Accounts.resend_code_to(params["pending"]), do: json(conn, sent_to)
  end

  def resend_code(conn, params) do
    with {:ok, sent} <- Accounts.resend_code(params["pending"], params["locale"] || "en"), do: json(conn, sent)
  end

  def sign_in_confirm(conn, params) do
    with {:ok, session} <- Accounts.sign_in_confirm(params["pending"], params["terms_version"]),
         do: json(conn, session)
  end

  def sign_in_confirm_for_device(conn, params) do
    with {:ok, code} <-
           Accounts.sign_in_confirm_for_device(params["pending"], params["challenge"], params["terms_version"]),
         do: json(conn, code)
  end

  def sign_in_with_device_code(conn, params) do
    with {:ok, session} <- Accounts.sign_in_with_device_code(params["code"], params["challenge"]),
         do: json(conn, session)
  end

  def sign_out(conn, params) do
    :ok = Accounts.sign_out(params["token"])
    json(conn, %{})
  end
end
