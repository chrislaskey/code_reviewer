defmodule Core.Accounts do
  @moduledoc """
  Signing in with a code mailed to an address, as `accounts::sign_in` and
  its confirm do. The backend never sends mail: it answers what to send,
  and Rust sends it.
  """
  alias Core.App.{Communities, Users}
  alias Core.Data.{AccountsTokens, AccountsUsers, AppSyncVersions, AppUsers}
  alias Core.Helpers.{Ids, Random, Time}
  alias Core.Repo

  @max_attempts 5

  @doc "The account of the user, as `accounts::users::get` answers it."
  def users_get(user_id) do
    case Ids.cast(user_id) && AccountsUsers.get(user_id) do
      %AccountsUsers{} = account -> {:ok, AccountsUsers.to_wire(account)}
      _ -> {:error, :not_found}
    end
  end

  @doc "Starts a sign-in for an address, signing it up when it is new and sign-up is enabled."
  def sign_in(email, sign_up_enabled, locale) do
    case AccountsUsers.checked_email(email) do
      nil ->
        {:error, {:invalid, %{"email" => ["is not an email address"]}}}

      email ->
        Repo.transaction(fn ->
          case AccountsUsers.get_by_email(email) do
            %AccountsUsers{trial_expires_at: nil} = account ->
              code_sent(account.id, email, locale)

            %AccountsUsers{} ->
              %{"pending" => Random.hex(32), "mail" => nil}

            nil when sign_up_enabled ->
              # Only a sign-up writes a synced row: its user
              AppSyncVersions.lock()
              now = Time.now()

              account =
                AccountsUsers.create(%AccountsUsers{id: Ids.new(), email: email, inserted_at: now, updated_at: now})

              Users.create_in(account.id, email)
              code_sent(account.id, email, locale)

            nil ->
              %{
                "pending" => Random.hex(32),
                "mail" => %{"kind" => "no_account", "to" => email, "locale" => locale, "code" => nil}
              }
          end
        end)
    end
  end

  defp code_sent(user_id, email, locale) do
    AccountsTokens.delete_of_user(user_id, AccountsTokens.context_sign_in_code())
    pending = Random.hex(32)
    code = Random.digits(6)
    AccountsTokens.create(user_id, AccountsTokens.context_sign_in_code(), pending, code)
    %{"pending" => pending, "mail" => %{"kind" => "sign_in_code", "to" => email, "locale" => locale, "code" => code}}
  end

  @doc """
  Takes the code mailed for a pending sign-in, as `accounts::sign_in_with_code`
  does: the pending token of the last step, and whether that step makes
  the account. A wrong code counts; after five the pending sign-in is gone.
  """
  def sign_in_with_code(pending, code) do
    case AccountsTokens.get_valid(pending || "", AccountsTokens.context_sign_in_code()) do
      nil ->
        {:error, :wrong_code}

      row ->
        if Core.Helpers.Sha256.hex(String.trim(code || "")) == row.code_hash do
          Repo.transaction(fn ->
            AccountsTokens.delete(row)
            AccountsTokens.delete_of_user(row.user_id, AccountsTokens.context_sign_in_confirm())
            confirm_pending = Random.hex(32)
            AccountsTokens.create(row.user_id, AccountsTokens.context_sign_in_confirm(), confirm_pending)
            %{"pending" => confirm_pending, "new_account" => is_nil(AccountsUsers.get(row.user_id).confirmed_at)}
          end)
        else
          attempts = AccountsTokens.update_attempts(row)
          if attempts >= @max_attempts, do: AccountsTokens.delete(row)
          {:error, :wrong_code}
        end
    end
  end

  @doc """
  Finishes a sign-in whose code matched, as `accounts::sign_in_confirm`
  does: a new token for the user. A first sign-in confirms the account
  and makes the user's personal community.
  """
  def sign_in_confirm(pending) do
    case AccountsTokens.get_valid(pending || "", AccountsTokens.context_sign_in_confirm()) do
      nil -> {:error, :sign_in_expired}
      row -> Repo.transaction(fn -> confirmed(row) end)
    end
  end

  defp confirmed(row) do
    AccountsTokens.delete(row)
    account = AccountsUsers.get(row.user_id)

    account =
      if account.confirmed_at do
        account
      else
        AppSyncVersions.lock()
        now = Time.now()
        Communities.create_personal_in(row.user_id)
        AccountsUsers.update(%{account | confirmed_at: now, updated_at: now})
      end

    token = Random.hex(32)
    AccountsTokens.create(account.id, AccountsTokens.context_api(), token)
    %{"user" => AppUsers.to_wire(AppUsers.get(account.id)), "email" => account.email, "token" => token}
  end

  @doc "Forgets a token."
  def sign_out(token) when is_binary(token) do
    AccountsTokens.delete_by_token(token)
    :ok
  end
end
