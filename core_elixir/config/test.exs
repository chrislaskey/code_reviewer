import Config

url = System.get_env("TEST_DATABASE_URL", "postgres://postgres@localhost/core_test")

config :core, Core.Repo,
  url: url,
  pool_size: 10

config :core, Core.EventStore, url: url, pool_size: 5

config :core, Oban, testing: :manual

config :core, CoreWeb.Endpoint,
  http: [ip: {127, 0, 0, 1}, port: 4101],
  secret_key_base: String.duplicate("test", 16),
  server: false

config :core, :service_token, "test-service-token"

config :logger, level: :warning
