-- A person's account, see src/accounts/users.rs: their email, when they
-- proved it, and their Stripe customer. Server only, like every
-- accounts_ table. The user's row the apps sync, app_users, has the
-- same id.
--
-- A trial user, see src/trial_users.rs, is an account made on the web
-- with no sign-up; `trial_expires_at` says until when, null otherwise.
CREATE TABLE accounts_users (
  id uuid PRIMARY KEY,
  email text NOT NULL,
  confirmed_at timestamptz,
  trial_expires_at timestamptz,
  stripe_customer_id text,
  inserted_at timestamptz NOT NULL,
  updated_at timestamptz NOT NULL,
  deleted_at timestamptz
);

-- Partial, so a deleted account's email can register again
CREATE UNIQUE INDEX accounts_users_email ON accounts_users (email) WHERE deleted_at IS NULL;
CREATE UNIQUE INDEX accounts_users_stripe_customer_id ON accounts_users (stripe_customer_id);

-- Tokens, see src/tokens.rs, kept as their hash: 'api' for the apps'
-- bearer tokens, 'session' for the browser's cookie, 'sign_in_code' for
-- a sign-in that passed the password and waits for its emailed code,
-- whose hash and wrong attempts are `code_hash` and `attempts`, and
-- 'device_code' for a finished browser sign-in a device collects once.
-- `sent_to` is the address a pending code went to when it is not yet the
-- account's, as for a trial user signing up.
CREATE TABLE accounts_tokens (
  id uuid PRIMARY KEY,
  user_id uuid NOT NULL REFERENCES accounts_users (id) ON DELETE CASCADE,
  context text NOT NULL,
  token_hash text NOT NULL UNIQUE,
  code_hash text,
  attempts integer NOT NULL DEFAULT 0,
  sent_to text,
  inserted_at timestamptz NOT NULL
);

CREATE INDEX accounts_tokens_user_id ON accounts_tokens (user_id);

-- Passwords, see src/credentials.rs: one row per account that has one,
-- holding the hash.
CREATE TABLE accounts_credentials (
  user_id uuid PRIMARY KEY REFERENCES accounts_users (id) ON DELETE CASCADE,
  password_hash text NOT NULL,
  inserted_at timestamptz NOT NULL,
  updated_at timestamptz NOT NULL
);
