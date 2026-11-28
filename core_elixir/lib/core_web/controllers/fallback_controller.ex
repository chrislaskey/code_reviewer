defmodule CoreWeb.FallbackController do
  @moduledoc "Turns an operation's `{:error, …}` into the protocol's error answer: a status and a stable code."
  use Phoenix.Controller, formats: [:json]
  import Plug.Conn

  def call(conn, {:error, :unknown_token}), do: error(conn, 401, "unknown_token", "the token is not known")
  def call(conn, {:error, :account_deleted}), do: error(conn, 410, "account_deleted", "this account was deleted")
  def call(conn, {:error, :not_a_member}), do: error(conn, 403, "not_a_member", "not a member of the community")
  def call(conn, {:error, :not_found}), do: error(conn, 404, "not_found", "not found")
  def call(conn, {:error, :exists}), do: error(conn, 409, "exists", "it exists already")
  def call(conn, {:error, :full}), do: error(conn, 503, "full", "Trials are full right now; sign up instead")

  def call(conn, {:error, :trial_has_no_code}),
    do: error(conn, 422, "trial_has_no_code", "A trial is deleted without a code")

  def call(conn, {:error, :not_a_trial}), do: error(conn, 422, "not_a_trial", "Only a trial signs up this way")

  def call(conn, {:error, :trial_writes_full}),
    do: error(conn, 403, "trial_writes_full", "This trial has reached its limit. Sign up to keep going.")

  def call(conn, {:error, :sign_in_expired}), do: error(conn, 422, "sign_in_expired", "the sign-in expired")

  def call(conn, {:error, :unknown_device_code}),
    do: error(conn, 422, "unknown_device_code", "the device code is wrong or expired")

  def call(conn, {:error, :wrong_code}),
    do: error(conn, 422, "wrong_code", "the code is wrong", %{"code" => ["is wrong"]})

  def call(conn, {:error, :invalid_continuation}),
    do: error(conn, 400, "invalid_continuation", "invalid sync continuation")

  def call(conn, {:error, {:invalid, fields}}), do: error(conn, 400, "invalid", "invalid", fields)
  def call(conn, {:error, {:refused, reason}}), do: error(conn, 400, "invalid", reason)

  @doc "The protocol's error answer."
  def error(conn, status, code, message, fields \\ nil) do
    body = %{"code" => code, "message" => message}
    body = if fields, do: Map.put(body, "fields", fields), else: body

    conn
    |> put_status(status)
    |> json(%{"error" => body})
  end
end
