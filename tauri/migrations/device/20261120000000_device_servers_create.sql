-- The servers this device knows, see src/device_accounts/servers.rs and
-- docs/design/multi-server.md: the build's default server, whose URL is
-- found at runtime and never stored, and any other the person added.
-- What a server said of itself and whether the default server lists it
-- are kept for display.
CREATE TABLE device_servers (
  id TEXT PRIMARY KEY NOT NULL,
  url TEXT UNIQUE,
  name TEXT,
  display_name TEXT,
  app_identifier TEXT,
  protocol_version INTEGER,
  listed TEXT NOT NULL CHECK (listed IN ('default', 'listed', 'unlisted')),
  checked_at TEXT,
  CHECK ((url IS NULL) = (listed = 'default'))
);

CREATE UNIQUE INDEX device_servers_one_default ON device_servers (listed)
  WHERE listed = 'default';
