-- Events, see src/app/events.rs: one change a person made to a row, by
-- the user signed in on a device, or the user the token resolved to on
-- the server. On the server the log of every event answered; on a
-- device the queue of events not yet answered, and the conflicts the
-- person has not resolved. `attempts` counts the times the server or the
-- bucket said no to a queued event: at the limit it stops being tried
-- and waits for the person. 0 on the server, always.
CREATE TABLE app_events (
  id uuid PRIMARY KEY,
  user_id uuid NOT NULL,
  table_name text NOT NULL,
  row_id uuid NOT NULL,
  type text NOT NULL,
  base_version bigint NOT NULL,
  base jsonb NOT NULL,
  changes jsonb NOT NULL,
  made_at timestamptz NOT NULL,
  status text NOT NULL,
  conflicts jsonb,
  version bigint,
  last_error text,
  attempts bigint NOT NULL DEFAULT 0
);

CREATE INDEX app_events_row ON app_events (table_name, row_id);
CREATE INDEX app_events_status ON app_events (status, made_at, id);

-- A device's pull cursor, see src/sync.rs: one row, the feed and the
-- global sync version the device has pulled through, or, while a first
-- pull is not finished, the continuation of its next page instead. The
-- server has the table too and leaves it empty.
CREATE TABLE app_sync_state (
  id integer PRIMARY KEY,
  feed_id uuid,
  global_sync_version BIGINT CHECK (global_sync_version >= 0),
  continuation TEXT,
  CHECK ((feed_id IS NULL) = (global_sync_version IS NULL))
);

-- The communities a device still has to ask the server for everything
-- in, see src/app/sync.rs: one row per community, written when a
-- membership of the device's user arrives and removed once the backfill
-- is done. The server has the table and leaves it empty.
CREATE TABLE app_sync_backfills (
  community_id uuid PRIMARY KEY,
  -- The continuation of the next page, once one page is in
  continuation TEXT
);

-- One row per purge run, see src/app/purge.rs: the horizon, the oldest
-- deletion still kept after it. Server only; a device whose cursor is
-- older than the latest horizon resyncs from nothing.
CREATE TABLE app_purges (
  id uuid PRIMARY KEY,
  horizon timestamptz NOT NULL,
  minimum_global_sync_version BIGINT NOT NULL,
  inserted_at timestamptz NOT NULL
);

-- One server feed. Its version and the change it names commit together.
CREATE TABLE app_sync_versions (
  id INTEGER PRIMARY KEY CHECK (id = 1),
  feed_id uuid NOT NULL,
  global_sync_version BIGINT NOT NULL DEFAULT 0 CHECK (global_sync_version >= 0),
  minimum_global_sync_version BIGINT NOT NULL DEFAULT 0
    CHECK (minimum_global_sync_version BETWEEN 0 AND global_sync_version)
);

INSERT INTO app_sync_versions (id, feed_id) VALUES (1, gen_random_uuid());

CREATE TABLE app_sync_changes (
  global_sync_version BIGINT PRIMARY KEY NOT NULL CHECK (global_sync_version > 0),
  table_name TEXT NOT NULL,
  row_id uuid NOT NULL,
  written_at timestamptz NOT NULL
);

CREATE INDEX app_sync_changes_written_at ON app_sync_changes (written_at, global_sync_version);
