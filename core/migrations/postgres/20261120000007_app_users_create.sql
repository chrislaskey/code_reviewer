-- The app's users, see src/app/users.rs: one row per account, with the
-- same id as its accounts_users row, synced to devices so a post can
-- show who wrote it. Only what the pages show syncs: the name, and the
-- public URL of the picture in object storage, see src/app/avatars.rs.
--
-- Ids are UUIDv7, made by whichever device writes the row; times are
-- kept to the microsecond. Every synced table ends in the sync columns,
-- see docs/design/offline-sync.md: the server's version of the row, 0
-- on a device's row not yet pushed; the time the server last wrote it,
-- the pull cursor, null until then; and the soft delete, with who made
-- it.
CREATE TABLE app_users (
  id uuid PRIMARY KEY,
  name text NOT NULL,
  avatar_url text,
  inserted_at timestamptz NOT NULL,
  updated_at timestamptz NOT NULL,
  version bigint NOT NULL DEFAULT 0,
  last_synced_at timestamptz,
  global_sync_version BIGINT CHECK (global_sync_version > 0),
  deleted_at timestamptz,
  deleted_by_id uuid
);

CREATE INDEX app_users_global_sync_version ON app_users (global_sync_version);
