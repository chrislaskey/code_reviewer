-- The app's users, see src/app/users.rs: one row per account, with the
-- same id as its accounts_users row, synced to devices so a post can
-- show who wrote it. Only what the pages show syncs: the name, and the
-- public URL of the picture in object storage, see src/app/avatars.rs.
--
-- Ids are UUIDv7 text, made by whichever device writes the row; times
-- are RFC 3339 text in UTC, as sqlx writes them. Every synced table ends
-- in the sync columns, see docs/design/offline-sync.md: the server's
-- version of the row, 0 on a device's row not yet pushed; the time the
-- server last wrote it, the pull cursor, null until then; and the soft
-- delete, with who made it.
CREATE TABLE app_users (
  id TEXT PRIMARY KEY NOT NULL,
  name TEXT NOT NULL,
  avatar_url TEXT,
  inserted_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  version INTEGER NOT NULL DEFAULT 0,
  last_synced_at TEXT,
  deleted_at TEXT,
  deleted_by_id TEXT
);

CREATE INDEX app_users_last_synced_at ON app_users (last_synced_at, id);
