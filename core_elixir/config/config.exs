import Config

config :core_elixir, ecto_repos: [CoreElixir.Repo]
config :core_elixir, event_stores: [CoreElixir.EventStore]

config :core_elixir, CoreElixir.Repo, migration_timestamps: [type: :utc_datetime_usec]

config :core_elixir, CoreElixir.EventStore,
  serializer: Commanded.Serialization.JsonSerializer,
  schema: "event_store"

config :core_elixir, CoreElixir.CommandedApplication,
  event_store: [
    adapter: Commanded.EventStore.Adapters.EventStore,
    event_store: CoreElixir.EventStore
  ],
  pubsub: :local,
  registry: :local

config :commanded_ecto_projections, repo: CoreElixir.Repo

config :core_elixir, Oban,
  repo: CoreElixir.Repo,
  queues: [maintenance: 1],
  plugins: [{Oban.Plugins.Cron, crontab: [{"15 3 * * *", CoreElixir.Jobs.Purge}]}]

config :logger, level: :info

import_config "#{config_env()}.exs"
