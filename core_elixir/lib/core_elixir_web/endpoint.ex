defmodule CoreElixirWeb.Endpoint do
  use Phoenix.Endpoint, otp_app: :core_elixir

  plug Plug.RequestId
  plug Plug.Telemetry, event_prefix: [:phoenix, :endpoint]
  plug Plug.Parsers, parsers: [:json], pass: ["application/json"], json_decoder: Phoenix.json_library()
  plug CoreElixirWeb.Router
end
