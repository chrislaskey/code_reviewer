defmodule App.Repo.Migrations.CreateAppUserProfiles do
  use Ecto.Migration

  def change do
    # A user's public details, one per user, see `App.Data.UserProfile`
    create table(:app_user_profiles, primary_key: false) do
      add :id, :uuid, primary_key: true
      add :user_id, references(:app_users, type: :uuid, on_delete: :delete_all), null: false
      add :display_name, :string
      add :bio, :text
      add :last_synced_at, :utc_datetime_usec, null: false
      add :version, :bigint, null: false, default: 1
      add :deleted_at, :utc_datetime_usec
      add :deleted_by_id, :uuid

      timestamps(type: :utc_datetime_usec)
    end

    create unique_index(:app_user_profiles, [:user_id])
    create index(:app_user_profiles, [:last_synced_at])
  end
end
