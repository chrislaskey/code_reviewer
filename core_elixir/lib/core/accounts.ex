defmodule Core.Accounts do
  @moduledoc """
  Signing in with a code mailed to an address, as `accounts::sign_in` and
  its confirm do. The backend never sends mail: it answers what to send,
  and Rust sends it.
  """
  alias Core.Accounts.Preferences
  alias Core.App.{Communities, Users}
  alias Core.Data.{AccountsTokens, AccountsUsers, AppSyncVersions, AppUsers, TrialUsers}
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

              Users.create_in(account.id, Users.name_from_email_in(email, account.id))
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

  @doc """
  `sign_in/3` from a trial user's browser, as `accounts::sign_in_from_trial`
  does. An address with a confirmed account gets a code for that
  account, and the trial is left as it is. Any other address is the
  trial user signing up: the code goes to it, and `sign_in_confirm`
  gives their account that address. Their made-up name gives way at once
  to one from the address.
  """
  def sign_in_from_trial(trial_id, email, locale) do
    with {:ok, trial} <- live_account(trial_id),
         %DateTime{} <- trial.trial_expires_at || {:error, :not_a_trial},
         email when is_binary(email) <-
           AccountsUsers.checked_email(email) || {:error, {:invalid, %{"email" => ["is not an email address"]}}} do
      Repo.transaction(fn ->
        case AccountsUsers.get_by_email(email) do
          %AccountsUsers{trial_expires_at: %DateTime{}} ->
            %{"pending" => Random.hex(32), "mail" => nil}

          %AccountsUsers{confirmed_at: %DateTime{}} = taken ->
            code_sent(taken.id, email, locale)

          _ ->
            AppSyncVersions.lock()
            Users.update_name_in(trial.id, Users.name_from_email_in(email, trial.id))
            code_sent(trial.id, email, locale, email)
        end
      end)
    end
  end

  defp live_account(id) do
    case Ids.cast(id) && AccountsUsers.get(id) do
      %AccountsUsers{deleted_at: nil} = account -> {:ok, account}
      _ -> {:error, :wrong_code}
    end
  end

  defp code_sent(user_id, email, locale, sent_to \\ nil) do
    AccountsTokens.delete_of_user(user_id, AccountsTokens.context_sign_in_code())
    pending = Random.hex(32)
    code = Random.digits(6)
    AccountsTokens.create(user_id, AccountsTokens.context_sign_in_code(), pending, code, sent_to)
    locale = Preferences.mail_locale(user_id, locale)
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

            AccountsTokens.create(
              row.user_id,
              AccountsTokens.context_sign_in_confirm(),
              confirm_pending,
              nil,
              row.sent_to
            )

            new_account = not is_nil(row.sent_to) or is_nil(AccountsUsers.get(row.user_id).confirmed_at)
            %{"pending" => confirm_pending, "new_account" => new_account}
          end)
        else
          attempts = AccountsTokens.update_attempts(row)
          if attempts >= @max_attempts, do: AccountsTokens.delete(row)
          {:error, :wrong_code}
        end
    end
  end

  @doc """
  The address a pending sign-in's code went to, as `accounts::resend_code_to`
  answers it, for Rust's limit on how often an address is mailed.
  """
  def resend_code_to(pending) do
    case AccountsTokens.get_valid(pending || "", AccountsTokens.context_sign_in_code()) do
      nil -> {:error, :wrong_code}
      row -> {:ok, %{"email" => row.sent_to || AccountsUsers.get(row.user_id).email}}
    end
  end

  @doc "A new code for a pending sign-in, and ten minutes again, as `accounts::resend_code` does: the mail to send."
  def resend_code(pending, locale) do
    case AccountsTokens.get_valid(pending || "", AccountsTokens.context_sign_in_code()) do
      nil ->
        {:error, :wrong_code}

      row ->
        code = AccountsTokens.update_code(row, Random.digits(6))
        email = row.sent_to || AccountsUsers.get(row.user_id).email
        locale = Preferences.mail_locale(row.user_id, locale)
        {:ok, %{"mail" => %{"kind" => "sign_in_code", "to" => email, "locale" => locale, "code" => code}}}
    end
  end

  @doc """
  Finishes a sign-in whose code matched, as `accounts::sign_in_confirm`
  does: a new token for the user. A first sign-in confirms the account
  and makes the user's personal community. Every sign-in records the
  terms the person accepted, the version Rust shows.
  """
  def sign_in_confirm(pending, terms_version) do
    with {:ok, row} <- confirm_row(pending) do
      Repo.transaction(fn ->
        account = confirmed(row, terms_version)
        token = Random.hex(32)
        AccountsTokens.create(account.id, AccountsTokens.context_api(), token)
        %{"user" => AppUsers.to_wire(AppUsers.get(account.id)), "email" => account.email, "token" => token}
      end)
    end
  end

  @doc """
  Finishes it for the device whose challenge it is, as
  `accounts::sign_in_confirm_for_device` does: a code the device
  collects with its verifier, and no token.
  """
  def sign_in_confirm_for_device(pending, challenge, terms_version) when is_binary(challenge) do
    with {:ok, row} <- confirm_row(pending) do
      Repo.transaction(fn ->
        account = confirmed(row, terms_version)
        code = Random.hex(16)
        AccountsTokens.create(account.id, AccountsTokens.context_device_code(), code <> challenge)
        %{"code" => code}
      end)
    end
  end

  def sign_in_confirm_for_device(_pending, _challenge, _terms_version),
    do: {:error, {:invalid, %{"challenge" => ["is needed"]}}}

  @doc """
  Exchanges a device code for the device's token, once, as
  `accounts::sign_in_with_device_code` does. Rust turns the device's
  verifier into its challenge, so the row is found by both.
  """
  def sign_in_with_device_code(code, challenge) when is_binary(code) and is_binary(challenge) do
    case AccountsTokens.get_valid(code <> challenge, AccountsTokens.context_device_code()) do
      nil ->
        {:error, :unknown_device_code}

      row ->
        Repo.transaction(fn ->
          AccountsTokens.delete(row)
          account = AccountsUsers.get(row.user_id)
          token = Random.hex(32)
          AccountsTokens.create(account.id, AccountsTokens.context_api(), token)
          %{"user" => AppUsers.to_wire(AppUsers.get(account.id)), "email" => account.email, "token" => token}
        end)
    end
  end

  def sign_in_with_device_code(_code, _challenge), do: {:error, :unknown_device_code}

  defp confirm_row(pending) do
    case AccountsTokens.get_valid(pending || "", AccountsTokens.context_sign_in_confirm()) do
      nil -> {:error, :sign_in_expired}
      row -> {:ok, row}
    end
  end

  defp confirmed(row, terms_version) do
    AccountsTokens.delete(row)
    account = AccountsUsers.get(row.user_id)
    now = Time.now()

    account =
      cond do
        row.sent_to ->
          trial_signed_up(account, row.sent_to, now)

        account.confirmed_at ->
          account

        true ->
          AppSyncVersions.lock()
          Communities.create_in(row.user_id, Communities.personal_name())
          %{account | confirmed_at: now}
      end

    AccountsUsers.update(%{account | terms_version: terms_version, terms_accepted_at: now, updated_at: now})
  end

  # A trial user's account as the person's own, once the code sent to
  # their address matched: the address, confirmed, the trial over, their
  # communities shown to everyone, and the trial's tokens forgotten for
  # the one this sign-in ends in. A sign-up that never proved the same
  # address gives way; a confirmed account with it does not.
  defp trial_signed_up(account, email, now) do
    if is_nil(account.trial_expires_at), do: Repo.rollback(:wrong_code)

    case AccountsUsers.get_by_email(email) do
      nil -> :ok
      %AccountsUsers{confirmed_at: nil} = other -> AccountsUsers.delete_unconfirmed(other)
      _confirmed -> Repo.rollback(:wrong_code)
    end

    AppSyncVersions.lock()
    TrialUsers.touch_communities_of(account.id)
    AccountsTokens.delete_of_user(account.id, AccountsTokens.context_api())
    %{account | email: email, confirmed_at: now, trial_expires_at: nil}
  end

  @doc "Forgets a token."
  def sign_out(token) when is_binary(token) do
    AccountsTokens.delete_by_token(token)
    :ok
  end
end
