-- What a person chooses on a device, see src/device_settings.rs: one row
-- per setting, its value as JSON, so a new setting needs no migration.
-- Never synced; the server has the table and leaves it empty.
CREATE TABLE device_settings (
  key text PRIMARY KEY,
  value jsonb NOT NULL
);
