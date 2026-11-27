import Config

config :core_elixir, CoreElixir.Repo,
  url: System.get_env("DATABASE_URL", "postgres://postgres@localhost/core_elixir_dev"),
  pool_size: 10

config :core_elixir, CoreElixir.EventStore,
  url: System.get_env("DATABASE_URL", "postgres://postgres@localhost/core_elixir_dev"),
  pool_size: 5

config :core_elixir, :http, port: 4100, service_token: "dev-service-token"
