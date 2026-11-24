-- Plans, the stores' subscriptions as the server last heard them, see
-- src/accounts/subscriptions.rs. Server only. Stripe keeps the history,
-- so a user's rows go with them. A plan change that waits for the end
-- of the period paid for is `scheduled_price_id` and `scheduled_at`.
CREATE TABLE accounts_subscriptions (
  id uuid PRIMARY KEY,
  user_id uuid NOT NULL REFERENCES accounts_users (id) ON DELETE CASCADE,
  product_id uuid NOT NULL REFERENCES accounts_products (id) ON DELETE CASCADE,
  price_id uuid NOT NULL REFERENCES accounts_prices (id) ON DELETE CASCADE,
  store text NOT NULL,
  store_id text NOT NULL,
  status text NOT NULL,
  current_period_end timestamptz,
  cancel_at_period_end boolean NOT NULL,
  scheduled_price_id uuid REFERENCES accounts_prices (id) ON DELETE SET NULL,
  scheduled_at timestamptz,
  ended_at timestamptz,
  inserted_at timestamptz NOT NULL,
  updated_at timestamptz NOT NULL,
  UNIQUE (store, store_id)
);

CREATE INDEX accounts_subscriptions_user_id ON accounts_subscriptions (user_id);
