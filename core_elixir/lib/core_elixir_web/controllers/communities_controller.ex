defmodule CoreElixirWeb.CommunitiesController do
  use CoreElixirWeb, :controller
  alias CoreElixir.App.Communities

  def list(conn, params), do: json(conn, Communities.list(params["actor_id"]))
end
