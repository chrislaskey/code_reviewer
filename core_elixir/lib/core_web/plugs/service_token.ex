defmodule CoreWeb.Plugs.ServiceToken do
  @moduledoc "Refuses any request without the service token Axum and the backend share."
  import Plug.Conn

  def init(opts), do: opts

  def call(conn, _opts) do
    expected = "Bearer " <> Application.fetch_env!(:core, :service_token)

    case get_req_header(conn, "authorization") do
      [^expected] -> conn
      _ -> conn |> CoreWeb.FallbackController.error(401, "unauthorized", "a wrong service token") |> halt()
    end
  end
end
