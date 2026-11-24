defmodule Mix.Tasks.App.Migrations do
  @shortdoc "Exports the migrations of synced tables as SQLite files for clients"

  @moduledoc """
  Writes `priv/client_migrations/<version>_<name>.sql` for every
  migration that touches a synced table, see `App.ClientMigrations`. Run
  it after adding or changing a migration under `priv/repo/migrations`,
  and commit the files. `mix test` fails while a file is missing or stale.

      $ mix app.migrations           # write every missing or stale file
      $ mix app.migrations --check   # report the files out of date, exit 1 if any
  """

  use Mix.Task

  @impl true
  def run(args) do
    Mix.Task.run("app.config")
    {:ok, _apps} = Application.ensure_all_started(:ecto_sql)
    {:ok, _apps} = App.Repo.__adapter__().ensure_all_started(App.Repo.config(), :temporary)

    if "--check" in args do
      case App.ClientMigrations.check() do
        [] ->
          Mix.shell().info("every client migration is up to date")

        problems ->
          Enum.each(problems, &Mix.shell().error(App.ClientMigrations.describe(&1)))
          exit({:shutdown, 1})
      end
    else
      for {status, file} <- App.ClientMigrations.write() do
        Mix.shell().info("#{status} #{App.ClientMigrations.dir()}/#{file}")
      end
    end
  end
end
