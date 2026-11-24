-- The Stripe events already handled, see src/accounts/stripe_events.rs:
-- one row per event, so a repeat is skipped. Server only.
CREATE TABLE accounts_stripe_events (
  id TEXT PRIMARY KEY NOT NULL,
  type TEXT NOT NULL,
  handled_at TEXT NOT NULL
);
