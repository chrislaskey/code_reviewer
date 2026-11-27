defmodule Core.Migrations.Rust do
  @moduledoc """
  The Rust core's PostgreSQL migrations, `core/migrations/postgres/*.sql`,
  as Ecto migrations with the same versions. They are read when this
  module compiles and kept in it, so a release needs no copy of the
  files. The backend's schema is the Rust server's, table for table.
  """

  @directory Path.expand("../../../../core/migrations/postgres", __DIR__)
  @files @directory |> Path.join("*.sql") |> Path.wildcard() |> Enum.sort()

  for file <- @files do
    @external_resource file
  end

  @doc false
  # Recompiles when a migration file is added or removed
  def __mix_recompile__?, do: @files != @directory |> Path.join("*.sql") |> Path.wildcard() |> Enum.sort()

  migrations =
    for file <- @files do
      [version | _] = file |> Path.basename() |> String.split("_")
      module = Module.concat(__MODULE__, "V" <> version)
      statements = file |> File.read!() |> Core.Migrations.Statements.split()

      defmodule module do
        @moduledoc false
        use Ecto.Migration
        @statements statements

        def up, do: Enum.each(@statements, &execute/1)
        def down, do: raise("the Rust migrations go one way; reset the database instead")
      end

      {String.to_integer(version), module}
    end

  if migrations == [], do: raise("no Rust migrations in #{@directory}")

  @doc "Every Rust migration, as `{version, module}`, oldest first."
  def list, do: unquote(Macro.escape(migrations))
end
