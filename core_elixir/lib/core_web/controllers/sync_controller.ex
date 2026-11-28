defmodule CoreWeb.SyncController do
  use CoreWeb, :controller
  alias Core.Sync

  def push(conn, params) do
    with {:ok, answers} <- Sync.push(params["actor_id"], params["events"] || [], params["uploaded"]),
         do: json(conn, answers)
  end

  def pull(conn, params) do
    with {:ok, page} <- Sync.pull(params["actor_id"], params), do: json(conn, page)
  end

  def backfill(conn, params) do
    with {:ok, page} <- Sync.backfill(params["actor_id"], params), do: json(conn, page)
  end
end
