-- Events, see src/app/events.rs: one change a person made to a row, by
-- the user signed in on a device, or the user the token resolved to on
-- the server. On the server the log of every event answered; on a
-- device the queue of events not yet answered, and the conflicts the
-- person has not resolved. `attempts` counts the times the server or the
-- bucket said no to a queued event: at the limit it stops being tried
-- and waits for the person. 0 on the server, always.
CREATE TABLE app_events (
  id TEXT PRIMARY KEY NOT NULL,
  user_id TEXT NOT NULL,
  table_name TEXT NOT NULL,
  row_id TEXT NOT NULL,
  type TEXT NOT NULL,
  base_version INTEGER NOT NULL,
  base TEXT NOT NULL,
  changes TEXT NOT NULL,
  made_at TEXT NOT NULL,
  status TEXT NOT NULL,
  conflicts TEXT,
  version INTEGER,
  last_error TEXT,
  attempts INTEGER NOT NULL DEFAULT 0
);

CREATE INDEX app_events_row ON app_events (table_name, row_id);
CREATE INDEX app_events_status ON app_events (status, made_at, id);

-- A device's pull cursor, see src/app/sync.rs: one row, the key of the
-- last row pulled. The server has the table too and leaves it empty.
CREATE TABLE app_sync_state (
  id INTEGER PRIMARY KEY NOT NULL,
  cursor_last_synced_at TEXT NOT NULL,
  cursor_id TEXT NOT NULL
);

-- The communities a device still has to ask the server for everything
-- in, see src/app/sync.rs: one row per community, written when a
-- membership of the device's user arrives and removed once the backfill
-- is done. The server has the table and leaves it empty.
CREATE TABLE app_sync_backfills (
  community_id TEXT PRIMARY KEY NOT NULL
);

-- One row per purge run, see src/app/purge.rs: the horizon, the oldest
-- deletion still kept after it. Server only; a device whose cursor is
-- older than the latest horizon resyncs from nothing.
CREATE TABLE app_purges (
  id TEXT PRIMARY KEY NOT NULL,
  horizon TEXT NOT NULL,
  inserted_at TEXT NOT NULL
);
