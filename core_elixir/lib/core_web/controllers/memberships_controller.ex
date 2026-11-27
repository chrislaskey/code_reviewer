defmodule CoreWeb.MembershipsController do
  use CoreWeb, :controller
  alias Core.App.Memberships

  def create(conn, params) do
    with {:ok, membership} <- Memberships.create(params["actor_id"], params["community_id"]), do: json(conn, membership)
  end

  def delete(conn, params) do
    with {:ok, _} <- Memberships.delete(params["actor_id"], params["community_id"]), do: json(conn, %{})
  end
end
