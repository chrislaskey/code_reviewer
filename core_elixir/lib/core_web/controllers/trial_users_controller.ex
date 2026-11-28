defmodule CoreWeb.TrialUsersController do
  use CoreWeb, :controller
  alias Core.Accounts.TrialUsers

  def count(conn, _params) do
    with {:ok, count} <- TrialUsers.count(), do: json(conn, count)
  end

  def create(conn, params) do
    with {:ok, session} <- TrialUsers.create(params["max"]), do: json(conn, session)
  end
end
