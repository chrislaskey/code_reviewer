defmodule Core.Accounts.TrialUsers do
  @moduledoc """
  Trial users, as `accounts::trial_users` in the Rust core: a person
  using the app on the web without signing up. `create` makes an account
  with a made-up email and `trial_expires_at`, a user, a community of
  their own and a token. Nobody else sees or joins their communities
  (`AppCommunities`, `Memberships`), they make `max_writes/0` writes and
  no more (`Core.Sync.Events.record/3`), and the hourly
  `Core.Jobs.TrialUsersDelete` removes them outright once the trial
  ended a day ago. Signing up is `Core.Accounts.sign_in_from_trial/3`.
  """
  alias Core.App.{Communities, Users}
  alias Core.Data.{AccountsTokens, AccountsUsers, AppSyncVersions, AppUsers, TrialUsers}
  alias Core.Helpers.{Ids, Random, Time}
  alias Core.Repo

  @shown_hours 24
  @kept_after_hours 24
  @max_writes 500
  @community_name "Guest"
  @name_prefix "guest_"
  @email_domain "example.com"

  @doc "How many writes a trial user makes, the two their first community takes included."
  def max_writes, do: @max_writes

  @doc "How many trial users there are, for the cap."
  def count, do: {:ok, %{"count" => TrialUsers.count()}}

  @doc """
  Makes a trial user and signs them in: the session for the browser's
  cookie. `:full` while `max` trial users exist.
  """
  def create(max) when is_integer(max) do
    Repo.transaction(fn ->
      AppSyncVersions.lock()
      if TrialUsers.count() >= max, do: Repo.rollback(:full)

      name = @name_prefix <> Random.letters_and_digits(11)
      now = Time.now()

      account =
        AccountsUsers.create(%AccountsUsers{
          id: Ids.new(),
          email: "#{name}@#{@email_domain}",
          trial_expires_at: DateTime.add(now, @shown_hours, :hour),
          inserted_at: now,
          updated_at: now
        })

      user = Users.create_in(account.id, name)
      Communities.create_in(user.id, @community_name)
      token = Random.hex(32)
      AccountsTokens.create(user.id, AccountsTokens.context_api(), token)
      %{"user" => AppUsers.to_wire(user), "email" => account.email, "token" => token}
    end)
  end

  def create(_max), do: {:error, {:invalid, %{"max" => ["is not a number"]}}}

  @doc "Whether the user is a trial user who has made `max_writes/0` writes."
  def writes_full?(user_id) do
    case AccountsUsers.get(user_id) do
      %AccountsUsers{trial_expires_at: %DateTime{}} -> TrialUsers.count_writes(user_id) >= @max_writes
      _ -> false
    end
  end

  @doc """
  Removes outright every trial user whose trial ended more than a day
  before `now`, with everything of theirs. Nobody else ever saw a row of
  theirs, so there is no device to tell. A trial that signed up is no
  longer one and is never touched.
  """
  def delete_expired(now) do
    ended_before = DateTime.add(now, -@kept_after_hours, :hour)

    Repo.transaction(fn ->
      AppSyncVersions.lock()
      %{"users" => TrialUsers.delete_expired_before(ended_before)}
    end)
  end
end
