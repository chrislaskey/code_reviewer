defmodule App.Repo.Migrations.CreateLinksTagsAndEvents do
  use Ecto.Migration

  # See docs/design/related-resources.md
  def up do
    create table(:app_links, primary_key: false) do
      add :id, :uuid, primary_key: true
      add :post_id, references(:app_posts, type: :uuid, on_delete: :delete_all), null: false
      add :label, :string, null: false
      add :url, :string, null: false
      add :last_synced_at, :utc_datetime_usec, null: false
      add :version, :bigint, null: false, default: 1
      add :deleted_at, :utc_datetime_usec
      add :deleted_by_id, :uuid

      timestamps(type: :utc_datetime_usec)
    end

    create index(:app_links, [:post_id])
    create index(:app_links, [:last_synced_at])

    create table(:app_tags, primary_key: false) do
      add :id, :uuid, primary_key: true
      add :owner_id, references(:app_users, type: :uuid, on_delete: :delete_all), null: false
      add :name, :string, null: false
      add :last_synced_at, :utc_datetime_usec, null: false
      add :version, :bigint, null: false, default: 1
      add :deleted_at, :utc_datetime_usec
      add :deleted_by_id, :uuid

      timestamps(type: :utc_datetime_usec)
    end

    create index(:app_tags, [:last_synced_at])

    # A name once per owner, ignoring case, among live tags. Raw SQL, since
    # an index on an expression is not something `create index` writes
    execute """
    CREATE UNIQUE INDEX "app_tags_owner_id_lower_name_index"
    ON "app_tags" (owner_id, lower(name)) WHERE deleted_at IS NULL
    """

    create table(:app_posts_tags, primary_key: false) do
      add :id, :uuid, primary_key: true
      add :post_id, references(:app_posts, type: :uuid, on_delete: :delete_all), null: false
      add :tag_id, references(:app_tags, type: :uuid, on_delete: :delete_all), null: false
      add :position, :integer, null: false
      add :note, :string
      add :last_synced_at, :utc_datetime_usec, null: false
      add :version, :bigint, null: false, default: 1
      add :deleted_at, :utc_datetime_usec
      add :deleted_by_id, :uuid

      timestamps(type: :utc_datetime_usec)
    end

    # Plain, not partial: tagging again revives the removed row
    create unique_index(:app_posts_tags, [:post_id, :tag_id])
    create index(:app_posts_tags, [:tag_id])
    create index(:app_posts_tags, [:last_synced_at])

    # Server-only: the event log of the resources that opt in, one stream
    # per resource, see "The event log" in docs/design/related-resources.md.
    # No foreign keys, since it points at many tables; the purge removes a
    # stream with its row
    create table(:app_events, primary_key: false) do
      add :id, :uuid, primary_key: true
      add :stream_table, :string, null: false
      add :stream_id, :uuid, null: false
      add :sequence, :bigint, null: false
      add :row_table, :string, null: false
      add :row_id, :uuid, null: false
      add :user_id, :uuid, null: false
      add :type, :string, null: false
      add :base_version, :bigint, null: false
      add :version, :bigint, null: false
      add :changes, :map, null: false
      add :conflicts, :map, null: false
      add :caused_by_table, :string
      add :caused_by_id, :uuid
      add :caused_by_version, :bigint

      timestamps(type: :utc_datetime_usec, updated_at: false)
    end

    create unique_index(:app_events, [:stream_table, :stream_id, :sequence])

    # A post's events so far move into its stream
    execute """
    INSERT INTO app_events (id, stream_table, stream_id, sequence, row_table, row_id, user_id,
      type, base_version, version, changes, conflicts, inserted_at)
    SELECT id, 'app_posts', post_id,
      ROW_NUMBER() OVER (PARTITION BY post_id ORDER BY inserted_at, id),
      'app_posts', post_id, user_id, type, base_version, version, changes, conflicts, inserted_at
    FROM app_post_events
    """

    drop table(:app_post_events)
  end

  def down do
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

    execute """
    INSERT INTO app_post_events (id, post_id, user_id, type, base_version, version, changes,
      conflicts, inserted_at)
    SELECT id, row_id, user_id, type, base_version, version, changes, conflicts, inserted_at
    FROM app_events WHERE row_table = 'app_posts'
    """

    drop table(:app_events)
    drop table(:app_posts_tags)
    drop table(:app_tags)
    drop table(:app_links)
  end
end
