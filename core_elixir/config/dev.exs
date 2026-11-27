import Config

config :core, Core.Repo,
  url: System.get_env("DATABASE_URL", "postgres://postgres@localhost/core_dev"),
  pool_size: 10

config :core, Core.EventStore,
  url: System.get_env("DATABASE_URL", "postgres://postgres@localhost/core_dev"),
  pool_size: 5

config :core, CoreWeb.Endpoint,
  http: [ip: {127, 0, 0, 1}, port: 4100],
  secret_key_base: String.duplicate("dev", 22),
  server: true

config :core, :service_token, "dev-service-token"
