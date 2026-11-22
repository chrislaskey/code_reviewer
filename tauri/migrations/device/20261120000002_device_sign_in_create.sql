-- The sign-in waiting on the browser, if one is, see src/accounts.rs:
-- the verifier the app keeps while the person signs in there, the email
-- it was started with, and when. One row at most; gone when the sign-in
-- finishes, is cancelled, or is stale at a launch.
CREATE TABLE device_sign_in (
  id INTEGER PRIMARY KEY NOT NULL CHECK (id = 1),
  verifier TEXT NOT NULL,
  email TEXT,
  started_at TEXT NOT NULL
);
