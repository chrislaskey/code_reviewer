defmodule CoreWeb.CommunitiesController do
  use CoreWeb, :controller
  alias Core.App.Communities

  def list(conn, params), do: json(conn, Communities.list(params["actor_id"]))
end
