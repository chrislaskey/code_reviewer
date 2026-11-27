import Config

config :core, ecto_repos: [Core.Repo]
config :core, event_stores: [Core.EventStore]

config :core, Core.Repo, migration_timestamps: [type: :utc_datetime_usec]

config :core, Core.EventStore,
  serializer: Commanded.Serialization.JsonSerializer,
  schema: "event_store"

config :core, Core.CommandedApplication,
  event_store: [
    adapter: Commanded.EventStore.Adapters.EventStore,
    event_store: Core.EventStore
  ],
  pubsub: :local,
  registry: :local

config :commanded_ecto_projections, repo: Core.Repo

config :core, Oban,
  repo: Core.Repo,
  queues: [maintenance: 1],
  plugins: [{Oban.Plugins.Cron, crontab: [{"15 3 * * *", Core.Jobs.Purge}]}]

config :core, CoreWeb.Endpoint,
  adapter: Bandit.PhoenixAdapter,
  render_errors: [formats: [json: CoreWeb.ErrorJSON], layout: false]

config :phoenix, :json_library, Jason

config :logger, level: :info

import_config "#{config_env()}.exs"
