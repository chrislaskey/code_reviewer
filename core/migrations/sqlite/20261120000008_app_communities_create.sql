-- Communities, see src/app/communities.rs: the soft boundary inside a
-- deployment, see docs/design/offline-sync.md. Any user creates one, and
-- every user sees every one, so a device can list them and join.
CREATE TABLE app_communities (
  id TEXT PRIMARY KEY NOT NULL,
  name TEXT NOT NULL,
  inserted_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  version INTEGER NOT NULL DEFAULT 0,
  global_sync_version INTEGER CHECK (global_sync_version > 0),
  deleted_at TEXT,
  deleted_by_id TEXT
);

CREATE INDEX app_communities_global_sync_version ON app_communities (global_sync_version);

-- Memberships, see src/app/memberships.rs: a user in a community, with a
-- role that is a string the app interprets. Leaving soft-deletes the
-- row; rejoining revives the same row, so the unique index is plain.
CREATE TABLE app_users_communities (
  id TEXT PRIMARY KEY NOT NULL,
  user_id TEXT NOT NULL REFERENCES app_users (id) ON DELETE CASCADE,
  community_id TEXT NOT NULL REFERENCES app_communities (id) ON DELETE CASCADE,
  role TEXT NOT NULL,
  inserted_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  version INTEGER NOT NULL DEFAULT 0,
  global_sync_version INTEGER CHECK (global_sync_version > 0),
  deleted_at TEXT,
  deleted_by_id TEXT
);

CREATE UNIQUE INDEX app_users_communities_user_community ON app_users_communities (user_id, community_id);
CREATE INDEX app_users_communities_community_id ON app_users_communities (community_id);
CREATE INDEX app_users_communities_global_sync_version ON app_users_communities (global_sync_version);
