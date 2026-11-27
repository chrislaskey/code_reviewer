import Config

config :core_elixir, CoreElixir.Repo,
  url: System.get_env("DATABASE_URL", "postgres://postgres@localhost/core_elixir_dev"),
  pool_size: 10

config :core_elixir, CoreElixir.EventStore,
  url: System.get_env("DATABASE_URL", "postgres://postgres@localhost/core_elixir_dev"),
  pool_size: 5

config :core_elixir, CoreElixirWeb.Endpoint,
  http: [ip: {127, 0, 0, 1}, port: 4100],
  secret_key_base: String.duplicate("dev", 22),
  server: true

config :core_elixir, :service_token, "dev-service-token"
