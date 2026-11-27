import Config

if config_env() == :prod do
  url = System.fetch_env!("DATABASE_URL")
  config :core_elixir, CoreElixir.Repo, url: url, pool_size: 10
  config :core_elixir, CoreElixir.EventStore, url: url, pool_size: 5

  config :core_elixir, CoreElixirWeb.Endpoint,
    http: [ip: {0, 0, 0, 0}, port: String.to_integer(System.get_env("PORT", "4100"))],
    secret_key_base: System.fetch_env!("SECRET_KEY_BASE"),
    server: true

  config :core_elixir, :service_token, System.fetch_env!("BACKEND_SERVICE_TOKEN")
end
