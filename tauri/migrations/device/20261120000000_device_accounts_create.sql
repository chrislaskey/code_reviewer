-- The accounts on this device, see src/device_accounts.rs: one row per
-- account, pointing at the account's own database file under accounts/.
-- No token and no user data: those are in the account's file. A local
-- user, see src/local_users.rs, is made by Skip on this device with no
-- account on the server; when they sign up or sign in, the row becomes
-- that account's and keeps its folder, so `file` need not carry the
-- account's own id.
CREATE TABLE device_accounts (
  user_id TEXT PRIMARY KEY NOT NULL,
  email TEXT NOT NULL,
  file TEXT NOT NULL,
  local INTEGER NOT NULL DEFAULT 0,
  signed_in INTEGER NOT NULL DEFAULT 0,
  last_active_at TEXT NOT NULL,
  active INTEGER NOT NULL DEFAULT 0
);
