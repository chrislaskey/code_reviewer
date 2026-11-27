import Config

url = System.get_env("TEST_DATABASE_URL", "postgres://postgres@localhost/core_elixir_test")

config :core_elixir, CoreElixir.Repo,
  url: url,
  pool_size: 10

config :core_elixir, CoreElixir.EventStore, url: url, pool_size: 5

config :core_elixir, Oban, testing: :manual

config :core_elixir, CoreElixirWeb.Endpoint,
  http: [ip: {127, 0, 0, 1}, port: 4101],
  secret_key_base: String.duplicate("test", 16),
  server: false

config :core_elixir, :service_token, "test-service-token"

config :logger, level: :warning
