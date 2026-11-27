import Config

if config_env() == :prod do
  url = System.fetch_env!("DATABASE_URL")
  config :core_elixir, CoreElixir.Repo, url: url, pool_size: 10
  config :core_elixir, CoreElixir.EventStore, url: url, pool_size: 5

  config :core_elixir, :http,
    port: String.to_integer(System.get_env("PORT", "4100")),
    service_token: System.fetch_env!("BACKEND_SERVICE_TOKEN")
end
