-- The accounts on this device, see src/device_accounts.rs: one row per
-- account, with an id made on the device that names its folder under
-- accounts/. No token and no user data: those are in the account's file.
-- An account is its server's user, so two servers made from one
-- database tell their users apart. A local user, see
-- src/app/local_users.rs, is made by Skip on this device with no account
-- on any server; its server is named when its records start moving into
-- an account, and the row becomes that account's, keeping its folder.
CREATE TABLE device_accounts (
  id TEXT PRIMARY KEY NOT NULL,
  server_id TEXT REFERENCES device_servers (id),
  user_id TEXT NOT NULL,
  email TEXT NOT NULL,
  local INTEGER NOT NULL DEFAULT 0,
  signed_in INTEGER NOT NULL DEFAULT 0,
  last_active_at TEXT NOT NULL,
  active INTEGER NOT NULL DEFAULT 0,
  CHECK (local = 1 OR server_id IS NOT NULL)
);

CREATE UNIQUE INDEX device_accounts_server_id_user_id ON device_accounts (server_id, user_id);
