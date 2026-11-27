-- Files, see src/app/files.rs and docs/design/files-communities.md: a row for
-- a file, whose bytes are in the server's bucket and in the account's
-- folder on a device, and a row for each community it is in. A file
-- belongs to its communities; `created_by_id` names who created it and
-- has no foreign key, so a file outlives its creator. `sha256` is the
-- bytes' checksum, lowercase hex, which the bucket checked. Only the
-- name changes.
CREATE TABLE app_files (
  id uuid PRIMARY KEY,
  created_by_id uuid NOT NULL,
  name text NOT NULL,
  content_type text NOT NULL,
  size bigint NOT NULL,
  sha256 text NOT NULL,
  inserted_at timestamptz NOT NULL,
  updated_at timestamptz NOT NULL,
  version bigint NOT NULL DEFAULT 0,
  last_synced_at timestamptz,
  global_sync_version BIGINT CHECK (global_sync_version > 0),
  deleted_at timestamptz,
  deleted_by_id uuid
);

CREATE INDEX app_files_global_sync_version ON app_files (global_sync_version);
CREATE INDEX app_files_sha256 ON app_files (sha256);

-- Taking a file out soft-deletes its row; putting it back revives the
-- same row, so the unique index is plain.
CREATE TABLE app_files_communities (
  id uuid PRIMARY KEY,
  file_id uuid NOT NULL REFERENCES app_files (id) ON DELETE CASCADE,
  community_id uuid NOT NULL REFERENCES app_communities (id) ON DELETE CASCADE,
  created_by_id uuid NOT NULL,
  inserted_at timestamptz NOT NULL,
  updated_at timestamptz NOT NULL,
  version bigint NOT NULL DEFAULT 0,
  last_synced_at timestamptz,
  global_sync_version BIGINT CHECK (global_sync_version > 0),
  deleted_at timestamptz,
  deleted_by_id uuid
);

CREATE UNIQUE INDEX app_files_communities_file_community ON app_files_communities (file_id, community_id);
CREATE INDEX app_files_communities_community_id ON app_files_communities (community_id);
CREATE INDEX app_files_communities_global_sync_version ON app_files_communities (global_sync_version);
