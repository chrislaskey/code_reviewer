defmodule CoreWeb.Endpoint do
  use Phoenix.Endpoint, otp_app: :core

  plug Plug.RequestId
  plug Plug.Telemetry, event_prefix: [:phoenix, :endpoint]
  plug Plug.Parsers, parsers: [:json], pass: ["application/json"], json_decoder: Phoenix.json_library()
  plug CoreWeb.Router
end
