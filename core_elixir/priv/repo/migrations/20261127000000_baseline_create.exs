defmodule CoreElixir.Repo.Migrations.BaselineCreate do
  @moduledoc """
  The tables of the sample backend. The synced tables match the Rust
  server's PostgreSQL schema column for column, so a device stores the
  rows as they arrive. The sync tables are the server's half of
  `core/migrations/postgres/*_app_sync_create.sql`.
  """
  use Ecto.Migration

  def up do
    execute """
    CREATE TABLE accounts_users (
      id uuid PRIMARY KEY,
      email text NOT NULL,
      confirmed_at timestamptz,
      trial_expires_at timestamptz,
      inserted_at timestamptz NOT NULL,
      updated_at timestamptz NOT NULL,
      deleted_at timestamptz
    )
    """

    execute "CREATE UNIQUE INDEX accounts_users_email ON accounts_users (email) WHERE deleted_at IS NULL"

    execute """
    CREATE TABLE accounts_tokens (
      id uuid PRIMARY KEY,
      user_id uuid NOT NULL REFERENCES accounts_users (id) ON DELETE CASCADE,
      context text NOT NULL,
      token_hash text NOT NULL UNIQUE,
      code_hash text,
      attempts integer NOT NULL DEFAULT 0,
      inserted_at timestamptz NOT NULL
    )
    """

    execute "CREATE INDEX accounts_tokens_user_id ON accounts_tokens (user_id)"

    for table <- ~w(app_users app_communities) do
      columns =
        case table do
          "app_users" -> "name text NOT NULL, avatar_url text,"
          "app_communities" -> "name text NOT NULL,"
        end

      execute """
      CREATE TABLE #{table} (
        id uuid PRIMARY KEY,
        #{columns}
        #{sync_columns()}
      )
      """

      execute "CREATE INDEX #{table}_global_sync_version ON #{table} (global_sync_version)"
    end

    execute """
    CREATE TABLE app_users_communities (
      id uuid PRIMARY KEY,
      user_id uuid NOT NULL REFERENCES app_users (id) ON DELETE CASCADE,
      community_id uuid NOT NULL REFERENCES app_communities (id) ON DELETE CASCADE,
      role text NOT NULL,
      #{sync_columns()}
    )
    """

    execute "CREATE UNIQUE INDEX app_users_communities_user_community ON app_users_communities (user_id, community_id)"
    execute "CREATE INDEX app_users_communities_community_id ON app_users_communities (community_id)"
    execute "CREATE INDEX app_users_communities_global_sync_version ON app_users_communities (global_sync_version)"

    execute """
    CREATE TABLE app_files (
      id uuid PRIMARY KEY,
      created_by_id uuid NOT NULL,
      name text NOT NULL,
      content_type text NOT NULL,
      size bigint NOT NULL,
      sha256 text NOT NULL,
      content text,
      #{sync_columns()}
    )
    """

    execute "CREATE INDEX app_files_global_sync_version ON app_files (global_sync_version)"
    execute "CREATE INDEX app_files_sha256 ON app_files (sha256)"

    execute """
    CREATE TABLE app_files_communities (
      id uuid PRIMARY KEY,
      file_id uuid NOT NULL REFERENCES app_files (id) ON DELETE CASCADE,
      community_id uuid NOT NULL REFERENCES app_communities (id) ON DELETE CASCADE,
      created_by_id uuid NOT NULL,
      #{sync_columns()}
    )
    """

    execute "CREATE UNIQUE INDEX app_files_communities_file_community ON app_files_communities (file_id, community_id)"
    execute "CREATE INDEX app_files_communities_community_id ON app_files_communities (community_id)"
    execute "CREATE INDEX app_files_communities_global_sync_version ON app_files_communities (global_sync_version)"

    # The answers to pushed events, by the event's id, so a retry gets the
    # same answer
    execute """
    CREATE TABLE app_events (
      id uuid PRIMARY KEY,
      user_id uuid NOT NULL,
      table_name text NOT NULL,
      row_id uuid NOT NULL,
      status text NOT NULL,
      version bigint,
      conflicts jsonb NOT NULL DEFAULT '{}',
      inserted_at timestamptz NOT NULL
    )
    """

    # One feed. Its version and the change it names commit together.
    execute """
    CREATE TABLE app_sync_versions (
      id integer PRIMARY KEY CHECK (id = 1),
      feed_id uuid NOT NULL,
      global_sync_version BIGINT NOT NULL DEFAULT 0 CHECK (global_sync_version >= 0),
      minimum_global_sync_version BIGINT NOT NULL DEFAULT 0
        CHECK (minimum_global_sync_version BETWEEN 0 AND global_sync_version)
    )
    """

    execute "INSERT INTO app_sync_versions (id, feed_id) VALUES (1, gen_random_uuid())"

    execute """
    CREATE TABLE app_sync_changes (
      global_sync_version BIGINT PRIMARY KEY CHECK (global_sync_version > 0),
      table_name text NOT NULL,
      row_id uuid NOT NULL,
      written_at timestamptz NOT NULL
    )
    """

    execute "CREATE INDEX app_sync_changes_written_at ON app_sync_changes (written_at, global_sync_version)"

    execute """
    CREATE TABLE projection_versions (
      projection_name text PRIMARY KEY,
      last_seen_event_number bigint,
      inserted_at timestamp without time zone NOT NULL,
      updated_at timestamp without time zone NOT NULL
    )
    """

    Oban.Migration.up()
  end

  def down do
    Oban.Migration.down(version: 1)

    for table <-
          ~w(projection_versions app_sync_changes app_sync_versions app_events app_files_communities app_files app_users_communities app_communities app_users accounts_tokens accounts_users) do
      execute "DROP TABLE #{table}"
    end
  end

  defp sync_columns do
    """
    inserted_at timestamptz NOT NULL,
    updated_at timestamptz NOT NULL,
    version bigint NOT NULL DEFAULT 0,
    last_synced_at timestamptz,
    global_sync_version BIGINT CHECK (global_sync_version > 0),
    deleted_at timestamptz,
    deleted_by_id uuid
    """
  end
end
