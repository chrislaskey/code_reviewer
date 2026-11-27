defmodule Core.Migrations do
  @moduledoc """
  The backend's migrations: the Rust core's PostgreSQL ones, then the
  Elixir backend's own. Run by `mix core.migrate`, and `migrate/0` in a
  release.
  """

  @elixir [{20_261_127_000_000, Core.Migrations.ObanAndProjectionsCreate}]

  @doc "Every migration, as `{version, module}`, oldest first."
  def list, do: Enum.sort(Core.Migrations.Rust.list() ++ @elixir)

  @doc "Runs every migration not yet run."
  def migrate do
    Application.load(:core)

    {:ok, _, _} =
      Ecto.Migrator.with_repo(Core.Repo, fn repo -> Ecto.Migrator.run(repo, list(), :up, all: true) end)

    :ok
  end
end
