-- Files, see src/app/files.rs and docs/design/files-communities.md: a row for
-- a file, whose bytes are in the server's bucket and in the account's
-- folder on a device, and a row for each community it is in. A file
-- belongs to its communities; `created_by_id` names who created it and
-- has no foreign key, so a file outlives its creator. `sha256` is the
-- bytes' checksum, lowercase hex, which the bucket checked. Only the
-- name changes.
CREATE TABLE app_files (
  id TEXT PRIMARY KEY NOT NULL,
  created_by_id TEXT NOT NULL,
  name TEXT NOT NULL,
  content_type TEXT NOT NULL,
  size INTEGER NOT NULL,
  sha256 TEXT NOT NULL,
  inserted_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  version INTEGER NOT NULL DEFAULT 0,
  global_sync_version INTEGER CHECK (global_sync_version > 0),
  deleted_at TEXT,
  deleted_by_id TEXT
);

CREATE INDEX app_files_global_sync_version ON app_files (global_sync_version);
CREATE INDEX app_files_sha256 ON app_files (sha256);

-- Taking a file out soft-deletes its row; putting it back revives the
-- same row, so the unique index is plain.
CREATE TABLE app_files_communities (
  id TEXT PRIMARY KEY NOT NULL,
  file_id TEXT NOT NULL REFERENCES app_files (id) ON DELETE CASCADE,
  community_id TEXT NOT NULL REFERENCES app_communities (id) ON DELETE CASCADE,
  created_by_id TEXT NOT NULL,
  inserted_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  version INTEGER NOT NULL DEFAULT 0,
  global_sync_version INTEGER CHECK (global_sync_version > 0),
  deleted_at TEXT,
  deleted_by_id TEXT
);

CREATE UNIQUE INDEX app_files_communities_file_community ON app_files_communities (file_id, community_id);
CREATE INDEX app_files_communities_community_id ON app_files_communities (community_id);
CREATE INDEX app_files_communities_global_sync_version ON app_files_communities (global_sync_version);
