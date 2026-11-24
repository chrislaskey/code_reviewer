defmodule App.Repo.Migrations.CreateCommunities do
  use Ecto.Migration

  def change do
    create table(:app_communities, primary_key: false) do
      add :id, :uuid, primary_key: true
      add :name, :string, null: false
      add :last_synced_at, :utc_datetime_usec, null: false
      add :version, :bigint, null: false, default: 1
      add :deleted_at, :utc_datetime_usec
      add :deleted_by_id, :uuid

      timestamps(type: :utc_datetime_usec)
    end

    create table(:app_users_communities, primary_key: false) do
      add :id, :uuid, primary_key: true
      add :user_id, references(:app_users, type: :uuid, on_delete: :delete_all), null: false

      add :community_id, references(:app_communities, type: :uuid, on_delete: :delete_all),
        null: false

      add :role, :string, null: false
      add :last_synced_at, :utc_datetime_usec, null: false
      add :version, :bigint, null: false, default: 1
      add :deleted_at, :utc_datetime_usec
      add :deleted_by_id, :uuid

      timestamps(type: :utc_datetime_usec)
    end

    create index(:app_users_communities, [:community_id])
    # Plain, not partial: rejoining revives the soft-deleted row
    create unique_index(:app_users_communities, [:user_id, :community_id])
  end
end
