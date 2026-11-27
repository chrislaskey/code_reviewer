-- Communities, see src/app/communities.rs: the soft boundary inside a
-- deployment, see docs/design/offline-sync.md. Any user creates one, and
-- every user sees every one, so a device can list them and join.
CREATE TABLE app_communities (
  id uuid PRIMARY KEY,
  name text NOT NULL,
  inserted_at timestamptz NOT NULL,
  updated_at timestamptz NOT NULL,
  version bigint NOT NULL DEFAULT 0,
  last_synced_at timestamptz,
  global_sync_version BIGINT CHECK (global_sync_version > 0),
  deleted_at timestamptz,
  deleted_by_id uuid
);

CREATE INDEX app_communities_global_sync_version ON app_communities (global_sync_version);

-- Memberships, see src/app/memberships.rs: a user in a community, with a
-- role that is a string the app interprets. Leaving soft-deletes the
-- row; rejoining revives the same row, so the unique index is plain.
CREATE TABLE app_users_communities (
  id uuid PRIMARY KEY,
  user_id uuid NOT NULL REFERENCES app_users (id) ON DELETE CASCADE,
  community_id uuid NOT NULL REFERENCES app_communities (id) ON DELETE CASCADE,
  role text NOT NULL,
  inserted_at timestamptz NOT NULL,
  updated_at timestamptz NOT NULL,
  version bigint NOT NULL DEFAULT 0,
  last_synced_at timestamptz,
  global_sync_version BIGINT CHECK (global_sync_version > 0),
  deleted_at timestamptz,
  deleted_by_id uuid
);

CREATE UNIQUE INDEX app_users_communities_user_community ON app_users_communities (user_id, community_id);
CREATE INDEX app_users_communities_community_id ON app_users_communities (community_id);
CREATE INDEX app_users_communities_global_sync_version ON app_users_communities (global_sync_version);
