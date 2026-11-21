-- One-time purchases, see src/accounts/checkouts.rs and
-- src/accounts/purchases.rs. Server only. Stripe keeps the history, so
-- a user's rows go with them.
CREATE TABLE accounts_checkouts (
  id TEXT PRIMARY KEY NOT NULL,
  user_id TEXT NOT NULL REFERENCES accounts_users (id) ON DELETE CASCADE,
  price_id TEXT NOT NULL REFERENCES accounts_prices (id) ON DELETE CASCADE,
  stripe_session_id TEXT NOT NULL UNIQUE,
  status TEXT NOT NULL,
  inserted_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

CREATE INDEX accounts_checkouts_user_id ON accounts_checkouts (user_id);

CREATE TABLE accounts_purchases (
  id TEXT PRIMARY KEY NOT NULL,
  user_id TEXT NOT NULL REFERENCES accounts_users (id) ON DELETE CASCADE,
  product_id TEXT NOT NULL REFERENCES accounts_products (id) ON DELETE CASCADE,
  price_id TEXT NOT NULL REFERENCES accounts_prices (id) ON DELETE CASCADE,
  checkout_id TEXT REFERENCES accounts_checkouts (id) ON DELETE SET NULL,
  store TEXT NOT NULL,
  store_id TEXT NOT NULL,
  amount INTEGER NOT NULL,
  currency TEXT NOT NULL,
  refunded_at TEXT,
  inserted_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  UNIQUE (store, store_id)
);

CREATE INDEX accounts_purchases_user_id ON accounts_purchases (user_id);
