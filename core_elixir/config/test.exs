import Config

url = System.get_env("TEST_DATABASE_URL", "postgres://postgres@localhost/core_elixir_test")

config :core_elixir, CoreElixir.Repo,
  url: url,
  pool_size: 10

config :core_elixir, CoreElixir.EventStore, url: url, pool_size: 5

config :core_elixir, Oban, testing: :manual

config :core_elixir, :http, port: 4101, service_token: "test-service-token", start: false

config :logger, level: :warning
