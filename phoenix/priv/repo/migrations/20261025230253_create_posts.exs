defmodule App.Repo.Migrations.CreatePosts do
  use Ecto.Migration

  def change do
    create table(:app_posts, primary_key: false) do
      add :id, :uuid, primary_key: true
      add :owner_id, references(:app_users, type: :uuid, on_delete: :delete_all), null: false
      add :body, :text, null: false
      add :last_synced_at, :utc_datetime_usec, null: false
      add :version, :bigint, null: false, default: 1
      add :deleted_at, :utc_datetime_usec
      add :deleted_by_id, :uuid

      timestamps(type: :utc_datetime_usec)
    end

    create index(:app_posts, [:owner_id])

    create table(:app_posts_communities, primary_key: false) do
      add :id, :uuid, primary_key: true
      add :post_id, references(:app_posts, type: :uuid, on_delete: :delete_all), null: false

      add :community_id, references(:app_communities, type: :uuid, on_delete: :delete_all),
        null: false

      # The membership the post was added under; the leave cascade and the
      # purge follow it
      add :users_communities_id,
          references(:app_users_communities, type: :uuid, on_delete: :delete_all),
          null: false

      add :last_synced_at, :utc_datetime_usec, null: false
      add :version, :bigint, null: false, default: 1
      add :deleted_at, :utc_datetime_usec
      add :deleted_by_id, :uuid

      timestamps(type: :utc_datetime_usec)
    end

    create index(:app_posts_communities, [:community_id])
    create index(:app_posts_communities, [:users_communities_id])
    # Plain, not partial: adding a post back revives the soft-deleted row
    create unique_index(:app_posts_communities, [:post_id, :community_id])

    # Server-only: every change to a post, see "Event-sourced resources" in
    # docs/design/offline-sync.md
    create table(:app_post_events, primary_key: false) do
      add :id, :uuid, primary_key: true
      add :post_id, references(:app_posts, type: :uuid, on_delete: :delete_all), null: false
      add :user_id, references(:app_users, type: :uuid, on_delete: :delete_all), null: false
      add :type, :string, null: false
      add :base_version, :bigint, null: false
      add :version, :bigint, null: false
      add :changes, :map, null: false
      add :conflicts, :map, null: false

      timestamps(type: :utc_datetime_usec, updated_at: false)
    end

    create index(:app_post_events, [:post_id, :inserted_at])
  end
end
