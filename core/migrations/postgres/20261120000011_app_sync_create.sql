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

-- A device's pull cursor, see src/app/sync.rs: one row, the key of the
-- last row pulled. The server has the table too and leaves it empty.
CREATE TABLE app_sync_state (
  id integer PRIMARY KEY,
  cursor_last_synced_at timestamptz NOT NULL,
  cursor_id uuid NOT NULL
);

-- The communities a device still has to ask the server for everything
-- in, see src/app/sync.rs: one row per community, written when a
-- membership of the device's user arrives and removed once the backfill
-- is done. The server has the table and leaves it empty.
CREATE TABLE app_sync_backfills (
  community_id uuid PRIMARY KEY
);

-- One row per purge run, see src/app/purge.rs: the horizon, the oldest
-- deletion still kept after it. Server only; a device whose cursor is
-- older than the latest horizon resyncs from nothing.
CREATE TABLE app_purges (
  id uuid PRIMARY KEY,
  horizon timestamptz NOT NULL,
  inserted_at timestamptz NOT NULL
);
