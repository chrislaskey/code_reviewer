defmodule CoreElixirWeb.FilesCommunitiesController do
  use CoreElixirWeb, :controller
  alias CoreElixir.App.FilesCommunities

  def create(conn, params) do
    with {:ok, row} <- FilesCommunities.create(params["actor_id"], params["file_id"], params["community_id"]),
         do: json(conn, row)
  end

  def delete(conn, params) do
    with {:ok, row} <- FilesCommunities.delete(params["actor_id"], params["file_id"], params["community_id"]),
         do: json(conn, row)
  end
end
