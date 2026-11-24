defmodule App.Repo.Migrations.CreateAppPurges do
  use Ecto.Migration

  def change do
    # One row per purge run with its horizon, the oldest last_synced_at
    # still intact after it, see `App.Data.Purge`. Server-only: the table
    # never reaches a client
    create table(:app_purges, primary_key: false) do
      add :id, :uuid, primary_key: true
      add :horizon, :utc_datetime_usec, null: false

      timestamps(type: :utc_datetime_usec, updated_at: false)
    end
  end
end
