defmodule Core.Migrations.ObanAndProjectionsCreate do
  @moduledoc """
  The tables only the Elixir backend has: Oban's jobs, and the checkpoints
  of Commanded's projectors. The event store makes its own schema.
  """
  use Ecto.Migration

  def up do
    Oban.Migration.up()

    execute """
    CREATE TABLE projection_versions (
      projection_name text PRIMARY KEY,
      last_seen_event_number bigint,
      inserted_at timestamp without time zone NOT NULL,
      updated_at timestamp without time zone NOT NULL
    )
    """
  end

  def down do
    execute "DROP TABLE projection_versions"
    Oban.Migration.down(version: 1)
  end
end
