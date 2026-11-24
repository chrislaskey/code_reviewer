-- One row per recurring job, see src/jobs.rs: when its last run started
-- and finished, and what it did or the error it stopped at. Not synced;
-- each deployment runs and records its own.
CREATE TABLE app_jobs (
  name TEXT PRIMARY KEY NOT NULL,
  started_at TEXT NOT NULL,
  finished_at TEXT,
  last_error TEXT,
  last_report TEXT
);
