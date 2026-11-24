-- What a person chooses for themselves, see src/accounts/preferences.rs:
-- one row per user and key, its value as JSON, so a new preference needs
-- no migration. The one accounts table that syncs, to its own user's
-- devices only, see docs/design/locales.md; no foreign key, since a
-- device's accounts_users is empty and a local user has no account, so
-- an account's deletion removes its rows itself.
CREATE TABLE accounts_preferences (
  id uuid PRIMARY KEY,
  user_id uuid NOT NULL,
  key text NOT NULL,
  value jsonb NOT NULL,
  inserted_at timestamptz NOT NULL,
  updated_at timestamptz NOT NULL,
  version bigint NOT NULL DEFAULT 0,
  last_synced_at timestamptz,
  deleted_at timestamptz,
  deleted_by_id uuid
);

CREATE UNIQUE INDEX accounts_preferences_user_id_key ON accounts_preferences (user_id, key);
CREATE INDEX accounts_preferences_last_synced_at ON accounts_preferences (last_synced_at, id);
