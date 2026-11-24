defmodule App.Repo.Migrations.CreateComments do
  use Ecto.Migration

  # See docs/design/comments.md
  def change do
    create table(:app_comments, primary_key: false) do
      add :id, :uuid, primary_key: true

      # The post in the community: a post in two communities has a thread
      # in each
      add :posts_communities_id,
          references(:app_posts_communities, type: :uuid, on_delete: :delete_all),
          null: false

      add :owner_id, references(:app_users, type: :uuid, on_delete: :delete_all), null: false
      add :body, :text, null: false
      add :last_synced_at, :utc_datetime_usec, null: false
      add :version, :bigint, null: false, default: 1
      add :deleted_at, :utc_datetime_usec
      add :deleted_by_id, :uuid

      timestamps(type: :utc_datetime_usec)
    end

    create index(:app_comments, [:posts_communities_id])
    create index(:app_comments, [:owner_id])
    create index(:app_comments, [:last_synced_at])
  end
end
