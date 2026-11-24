-- One row per recurring job, see src/jobs.rs: when its last run started
-- and finished, and what it did or the error it stopped at. Not synced;
-- each deployment runs and records its own.
CREATE TABLE app_jobs (
  name text PRIMARY KEY,
  started_at timestamptz NOT NULL,
  finished_at timestamptz,
  last_error text,
  last_report jsonb
);
