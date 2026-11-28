defmodule Core.Accounts.Password do
  @moduledoc """
  The signed-in user's password, as `accounts::password` in the Rust
  core: whether they have one, and their first or a new one. Rust hashes
  a password and checks one, with Argon2id; the backend keeps the hash
  it is given, and answers it for a sign-in.
  """
  alias Core.Accounts.Preferences
  alias Core.Data.{AccountsCredentials, AccountsUsers}
  alias Core.Helpers.Time

  @doc "Whether the user has a password."
  def get(user_id), do: {:ok, %{"is_set" => not is_nil(AccountsCredentials.get(user_id))}}

  @doc """
  Keeps the hash as the user's password, their first or in place of the
  one they had. Answers the mail that tells the account's email, which
  Rust sends.
  """
  def update(user_id, password_hash, locale) when is_binary(password_hash) and password_hash != "" do
    case AccountsUsers.get(user_id) do
      %AccountsUsers{deleted_at: nil} = account ->
        now = Time.now()

        AccountsCredentials.upsert(%AccountsCredentials{
          user_id: user_id,
          password_hash: password_hash,
          inserted_at: now,
          updated_at: now
        })

        locale = Preferences.mail_locale(user_id, locale)
        {:ok, %{"mail" => %{"kind" => "password_changed", "to" => account.email, "locale" => locale, "code" => nil}}}

      _ ->
        {:error, :sign_in_expired}
    end
  end

  def update(_user_id, _password_hash, _locale), do: {:error, {:invalid, %{"password_hash" => ["is needed"]}}}

  @doc """
  The account with the address that signs in with a password, and its
  hash: a confirmed account, no trial. Either is nil when there is none,
  and Rust refuses each way alike.
  """
  def get_hash(email) do
    email = AccountsUsers.checked_email(email)

    account =
      case email && AccountsUsers.get_by_email(email) do
        %AccountsUsers{confirmed_at: %DateTime{}, trial_expires_at: nil} = account -> account
        _ -> nil
      end

    credential = account && AccountsCredentials.get(account.id)
    {:ok, %{"user_id" => account && account.id, "password_hash" => credential && credential.password_hash}}
  end
end
