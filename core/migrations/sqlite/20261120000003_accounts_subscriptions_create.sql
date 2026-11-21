-- Plans, the stores' subscriptions as the server last heard them, see
-- src/accounts/subscriptions.rs. Server only. Stripe keeps the history,
-- so a user's rows go with them. A plan change that waits for the end
-- of the period paid for is `scheduled_price_id` and `scheduled_at`.
CREATE TABLE accounts_subscriptions (
  id TEXT PRIMARY KEY NOT NULL,
  user_id TEXT NOT NULL REFERENCES accounts_users (id) ON DELETE CASCADE,
  product_id TEXT NOT NULL REFERENCES accounts_products (id) ON DELETE CASCADE,
  price_id TEXT NOT NULL REFERENCES accounts_prices (id) ON DELETE CASCADE,
  store TEXT NOT NULL,
  store_id TEXT NOT NULL,
  status TEXT NOT NULL,
  current_period_end TEXT,
  cancel_at_period_end INTEGER NOT NULL,
  scheduled_price_id TEXT REFERENCES accounts_prices (id) ON DELETE SET NULL,
  scheduled_at TEXT,
  ended_at TEXT,
  inserted_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  UNIQUE (store, store_id)
);

CREATE INDEX accounts_subscriptions_user_id ON accounts_subscriptions (user_id);
