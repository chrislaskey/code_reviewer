-- One-time purchases, see src/accounts/checkouts.rs and
-- src/accounts/purchases.rs. Server only. Stripe keeps the history, so
-- a user's rows go with them.
CREATE TABLE accounts_checkouts (
  id uuid PRIMARY KEY,
  user_id uuid NOT NULL REFERENCES accounts_users (id) ON DELETE CASCADE,
  price_id uuid NOT NULL REFERENCES accounts_prices (id) ON DELETE CASCADE,
  stripe_session_id text NOT NULL UNIQUE,
  status text NOT NULL,
  inserted_at timestamptz NOT NULL,
  updated_at timestamptz NOT NULL
);

CREATE INDEX accounts_checkouts_user_id ON accounts_checkouts (user_id);

CREATE TABLE accounts_purchases (
  id uuid PRIMARY KEY,
  user_id uuid NOT NULL REFERENCES accounts_users (id) ON DELETE CASCADE,
  product_id uuid NOT NULL REFERENCES accounts_products (id) ON DELETE CASCADE,
  price_id uuid NOT NULL REFERENCES accounts_prices (id) ON DELETE CASCADE,
  checkout_id uuid REFERENCES accounts_checkouts (id) ON DELETE SET NULL,
  store text NOT NULL,
  store_id text NOT NULL,
  amount bigint NOT NULL,
  currency text NOT NULL,
  refunded_at timestamptz,
  inserted_at timestamptz NOT NULL,
  updated_at timestamptz NOT NULL,
  UNIQUE (store, store_id)
);

CREATE INDEX accounts_purchases_user_id ON accounts_purchases (user_id);
