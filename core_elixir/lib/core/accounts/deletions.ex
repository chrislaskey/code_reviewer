defmodule Core.Accounts.Deletions do
  @moduledoc """
  Deleting an account, by the person whose it is, as `accounts::deletions`
  in the Rust core, see `docs/design/accounts-delete.md`. A full user
  asks for a code, mailed as a sign-in's is, and gives it back to
  `create/3`, which deletes everything of theirs: each community nobody
  else is in, as an event in their name, and their membership of every
  other, so every device that pulled a row sees it go; their
  preferences; their user, named "Deleted user"; their staff row; and
  their account, so a token of theirs is answered `account_deleted`. A
  trial needs no code, and goes outright. The backend answers the mail
  for Rust to send, and Rust deletes the pictures.
  """
  alias Core.Accounts.Preferences
  alias Core.App.{FilesCommunities, Memberships}

  alias Core.Data.{
    AccountsPreferences,
    AccountsTokens,
    AccountsUsers,
    AppCommunities,
    AppSyncVersions,
    AppUsers,
    AppUsersCommunities,
    SyncedRow,
    TrialUsers
  }

  alias Core.Helpers.{Random, Sha256, Time}
  alias Core.Repo
  alias Core.Sync.{Answers, Events}

  @max_attempts 5
  @deleted_name "Deleted user"

  @doc "Mails the full user a code to confirm deleting their account, replacing any earlier one; ten minutes."
  def code_create(user_id, locale) do
    with {:ok, account} <- live_account(user_id) do
      if account.trial_expires_at do
        {:error, :trial_has_no_code}
      else
        Repo.transaction(fn ->
          AccountsTokens.delete_of_user(user_id, AccountsTokens.context_delete_code())
          code = Random.digits(6)
          AccountsTokens.create(user_id, AccountsTokens.context_delete_code(), token(user_id), code)
          locale = Preferences.mail_locale(user_id, locale)
          %{"mail" => %{"kind" => "delete_code", "to" => account.email, "locale" => locale, "code" => code}}
        end)
      end
    end
  end

  @doc """
  Deletes the user's account, with the code `code_create/2` mailed them,
  or none for a trial. A wrong code counts, as a sign-in's does; the
  fifth deletes the code. Answers the mail that says it is done, none
  for a trial.
  """
  def create(user_id, code, locale) do
    with {:ok, account} <- live_account(user_id) do
      if account.trial_expires_at do
        Repo.transaction(fn ->
          AppSyncVersions.lock()
          TrialUsers.delete_one(user_id)
          %{"mail" => nil}
        end)
      else
        with :ok <- code_matched(user_id, code) do
          # Their language, read before their preferences go
          locale = Preferences.mail_locale(user_id, locale)
          now = Time.now()

          {:ok, communities} =
            Repo.transaction(fn ->
              AppSyncVersions.lock()
              delete_in(user_id, now)
            end)

          for community_id <- communities, do: FilesCommunities.delete_with_community(user_id, community_id, now)
          {:ok, %{"mail" => %{"kind" => "account_deleted", "to" => account.email, "locale" => locale, "code" => nil}}}
        end
      end
    end
  end

  defp code_matched(user_id, code) do
    case AccountsTokens.get_valid(token(user_id), AccountsTokens.context_delete_code()) do
      nil ->
        {:error, :wrong_code}

      row ->
        if Sha256.hex(String.trim(code || "")) == row.code_hash do
          AccountsTokens.delete(row)
          :ok
        else
          if AccountsTokens.update_attempts(row) >= @max_attempts, do: AccountsTokens.delete(row)
          {:error, :wrong_code}
        end
    end
  end

  # Everything of the user's but their files' links, which go with the
  # communities after the commit. The ids of the communities deleted.
  defp delete_in(user_id, now) do
    communities =
      for membership <- AppUsersCommunities.list_live_of_user(user_id),
          reduce: [] do
        deleted ->
          members = AppUsersCommunities.list_live_of_community(membership.community_id)

          if Enum.all?(members, &(&1.user_id == user_id)) do
            community_delete_in(user_id, membership.community_id, members, now)
            [membership.community_id | deleted]
          else
            Memberships.delete_in(user_id, membership.community_id)
            deleted
          end
      end

    user_delete_in(user_id, now)
    AccountsPreferences.delete_of_user(user_id)
    Repo.query!("DELETE FROM ops_staff WHERE user_id = $1", [Ecto.UUID.dump!(user_id)])
    account = AccountsUsers.get(user_id)
    AccountsUsers.update(%{account | deleted_at: now, updated_at: now})
    communities
  end

  # The community soft-deleted as the user's event, and its memberships with it
  defp community_delete_in(user_id, community_id, members, now) do
    community = AppCommunities.get(community_id)
    table = AppCommunities.table()

    event =
      Events.update(table, community.id, community.version, %{"deleted_at" => nil}, %{"deleted_at" => Time.to_wire(now)})

    deleted = AppCommunities.update(%{SyncedRow.changed(community, now) | deleted_at: now, deleted_by_id: user_id})
    Events.record(user_id, event, Answers.applied(event["id"], deleted.version, %{}, nil))

    for member <- members do
      AppUsersCommunities.update(%{SyncedRow.changed(member, now) | deleted_at: now, deleted_by_id: user_id})
    end
  end

  # The user soft-deleted as their event: named `@deleted_name`, without a picture
  defp user_delete_in(user_id, now) do
    user = AppUsers.get(user_id)

    event =
      Events.update(
        AppUsers.table(),
        user.id,
        user.version,
        %{"name" => user.name, "avatar_url" => user.avatar_url, "deleted_at" => nil},
        %{"name" => @deleted_name, "avatar_url" => nil, "deleted_at" => Time.to_wire(now)}
      )

    deleted =
      AppUsers.update(%{
        SyncedRow.changed(user, now)
        | name: @deleted_name,
          avatar_url: nil,
          deleted_at: now,
          deleted_by_id: user_id
      })

    Events.record(user_id, event, Answers.applied(event["id"], deleted.version, %{}, nil))
  end

  defp live_account(user_id) do
    case Core.Helpers.Ids.cast(user_id) && AccountsUsers.get(user_id) do
      %AccountsUsers{deleted_at: nil} = account -> {:ok, account}
      _ -> {:error, :not_found}
    end
  end

  # The delete code is found by its user, as Rust's `delete_code_hash`
  defp token(user_id), do: "delete_code:#{user_id}"
end
