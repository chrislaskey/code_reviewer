-- What a person chooses for themselves, see src/accounts/preferences.rs:
-- one row per user and key, its value as JSON, so a new preference needs
-- no migration. The one accounts table that syncs, to its own user's
-- devices only, see docs/design/locales.md; no foreign key, since a
-- device's accounts_users is empty and a local user has no account, so
-- an account's deletion removes its rows itself.
CREATE TABLE accounts_preferences (
  id TEXT PRIMARY KEY NOT NULL,
  user_id TEXT NOT NULL,
  key TEXT NOT NULL,
  value TEXT NOT NULL,
  inserted_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  version INTEGER NOT NULL DEFAULT 0,
  global_sync_version INTEGER CHECK (global_sync_version > 0),
  deleted_at TEXT,
  deleted_by_id TEXT
);

CREATE UNIQUE INDEX accounts_preferences_user_id_key ON accounts_preferences (user_id, key);
CREATE INDEX accounts_preferences_global_sync_version ON accounts_preferences (global_sync_version);
