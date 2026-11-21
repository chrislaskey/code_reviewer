-- The Stripe events already handled, see src/accounts/stripe_events.rs:
-- one row per event, so a repeat is skipped. Server only.
CREATE TABLE accounts_stripe_events (
  id text PRIMARY KEY,
  type text NOT NULL,
  handled_at timestamptz NOT NULL
);
