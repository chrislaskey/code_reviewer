-- Stripe's catalog as the server last heard it, see
-- src/accounts/products.rs and src/accounts/prices.rs. Server only.
CREATE TABLE accounts_products (
  id TEXT PRIMARY KEY NOT NULL,
  stripe_id TEXT NOT NULL UNIQUE,
  name TEXT NOT NULL,
  description TEXT,
  active INTEGER NOT NULL,
  inserted_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

CREATE TABLE accounts_prices (
  id TEXT PRIMARY KEY NOT NULL,
  product_id TEXT NOT NULL REFERENCES accounts_products (id) ON DELETE CASCADE,
  stripe_id TEXT NOT NULL UNIQUE,
  lookup_key TEXT,
  amount INTEGER NOT NULL,
  currency TEXT NOT NULL,
  interval TEXT,
  active INTEGER NOT NULL,
  inserted_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

CREATE INDEX accounts_prices_product_id ON accounts_prices (product_id);
