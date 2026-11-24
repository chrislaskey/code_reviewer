-- Who is staff, see src/ops/staff.rs: one row per account that may use
-- the ops app. Server only.
CREATE TABLE ops_staff (
  user_id TEXT PRIMARY KEY NOT NULL REFERENCES accounts_users (id) ON DELETE CASCADE,
  inserted_at TEXT NOT NULL
);
