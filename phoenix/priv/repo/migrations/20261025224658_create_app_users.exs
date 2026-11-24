defmodule App.Repo.Migrations.CreateAppUsers do
  use Ecto.Migration

  def change do
    # The app's own users, same id as `users`, see `App.Data.AppUser`
    create table(:app_users, primary_key: false) do
      add :id, :uuid, primary_key: true
      add :email, :string, null: false, collate: :nocase
      add :joined_at, :utc_datetime_usec, null: false
      add :last_synced_at, :utc_datetime_usec, null: false
      add :version, :bigint, null: false, default: 1
      add :deleted_at, :utc_datetime_usec
      add :deleted_by_id, :uuid

      timestamps(type: :utc_datetime_usec)
    end
  end
end
