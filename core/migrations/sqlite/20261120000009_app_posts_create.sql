-- Posts, see src/app/posts.rs, each by its user.
CREATE TABLE app_posts (
  id TEXT PRIMARY KEY NOT NULL,
  user_id TEXT NOT NULL REFERENCES app_users (id) ON DELETE CASCADE,
  body TEXT NOT NULL,
  inserted_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  version INTEGER NOT NULL DEFAULT 0,
  last_synced_at TEXT,
  deleted_at TEXT,
  deleted_by_id TEXT
);

CREATE INDEX app_posts_user_id ON app_posts (user_id);
CREATE INDEX app_posts_inserted_at ON app_posts (inserted_at);
CREATE INDEX app_posts_last_synced_at ON app_posts (last_synced_at, id);

-- A post in a community, see src/app/posts_communities.rs, inserted
-- under the user's membership, which leaving follows. Removing
-- soft-deletes the row; inserting it again revives the same row, so the
-- unique index is plain.
CREATE TABLE app_posts_communities (
  id TEXT PRIMARY KEY NOT NULL,
  post_id TEXT NOT NULL REFERENCES app_posts (id) ON DELETE CASCADE,
  community_id TEXT NOT NULL REFERENCES app_communities (id) ON DELETE CASCADE,
  users_communities_id TEXT NOT NULL REFERENCES app_users_communities (id) ON DELETE CASCADE,
  inserted_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  version INTEGER NOT NULL DEFAULT 0,
  last_synced_at TEXT,
  deleted_at TEXT,
  deleted_by_id TEXT
);

CREATE UNIQUE INDEX app_posts_communities_post_community ON app_posts_communities (post_id, community_id);
CREATE INDEX app_posts_communities_community_id ON app_posts_communities (community_id);
CREATE INDEX app_posts_communities_users_communities_id ON app_posts_communities (users_communities_id);
CREATE INDEX app_posts_communities_last_synced_at ON app_posts_communities (last_synced_at, id);
