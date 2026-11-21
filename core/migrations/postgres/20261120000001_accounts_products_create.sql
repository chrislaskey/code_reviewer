-- Stripe's catalog as the server last heard it, see
-- src/accounts/products.rs and src/accounts/prices.rs. Server only.
CREATE TABLE accounts_products (
  id uuid PRIMARY KEY,
  stripe_id text NOT NULL UNIQUE,
  name text NOT NULL,
  description text,
  active boolean NOT NULL,
  inserted_at timestamptz NOT NULL,
  updated_at timestamptz NOT NULL
);

CREATE TABLE accounts_prices (
  id uuid PRIMARY KEY,
  product_id uuid NOT NULL REFERENCES accounts_products (id) ON DELETE CASCADE,
  stripe_id text NOT NULL UNIQUE,
  lookup_key text,
  amount bigint NOT NULL,
  currency text NOT NULL,
  interval text,
  active boolean NOT NULL,
  inserted_at timestamptz NOT NULL,
  updated_at timestamptz NOT NULL
);

CREATE INDEX accounts_prices_product_id ON accounts_prices (product_id);
