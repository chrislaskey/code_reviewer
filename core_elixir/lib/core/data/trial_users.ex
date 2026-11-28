defmodule Core.Data.TrialUsers do
  @moduledoc """
  What tells a trial user apart, as `data::trial_users` in the Rust
  core: an account with `trial_expires_at`. Each runs in the caller's
  transaction.
  """
  alias Core.Data.{AppCommunities, AppSyncChanges}
  alias Core.Repo

  @doc "The trial users there are, expired or not."
  def count do
    %{rows: [[count]]} = Repo.query!("SELECT COUNT(*) FROM accounts_users WHERE trial_expires_at IS NOT NULL")
    count
  end

  @doc "How many writes of the user the server has recorded."
  def count_writes(user_id) do
    %{rows: [[count]]} = Repo.query!("SELECT COUNT(*) FROM app_events WHERE user_id = $1", [Ecto.UUID.dump!(user_id)])
    count
  end

  @doc """
  Whether a user joining a community would put a trial user and anyone
  else in one community: the user is a trial and someone else is or was
  a member, or someone who is or was a member is a trial.
  """
  def join_is_walled_off?(user_id, community_id) do
    %{rows: [[walled_off]]} =
      Repo.query!(
        """
        SELECT EXISTS (
          SELECT 1 FROM app_users_communities uc
          WHERE uc.community_id = $2 AND uc.user_id != $1
            AND ((SELECT trial_expires_at FROM accounts_users WHERE id = $1) IS NOT NULL
              OR (SELECT trial_expires_at FROM accounts_users WHERE id = uc.user_id) IS NOT NULL))
        """,
        [Ecto.UUID.dump!(user_id), Ecto.UUID.dump!(community_id)]
      )

    walled_off
  end

  @doc """
  Marks a signed-up trial's communities changed, so the next pull of
  anyone brings them. The caller holds the feed lock.
  """
  def touch_communities_of(user_id) do
    %{rows: rows} =
      Repo.query!(
        "SELECT community_id FROM app_users_communities WHERE user_id = $1 ORDER BY community_id",
        [Ecto.UUID.dump!(user_id)]
      )

    for [id] <- rows do
      id = Ecto.UUID.load!(id)
      version = AppSyncChanges.record(AppCommunities.table(), id)

      Repo.query!("UPDATE app_communities SET version = version + 1, global_sync_version = $2 WHERE id = $1", [
        Ecto.UUID.dump!(id),
        version
      ])
    end

    :ok
  end

  @doc "Deletes outright the trial user, as `delete_expired_before/1` would later: a trial deleting itself."
  def delete_one(user_id) do
    trial = "SELECT id FROM accounts_users WHERE id = $1 AND trial_expires_at IS NOT NULL"
    id = Ecto.UUID.dump!(user_id)

    for statement <- [
          "DELETE FROM app_events WHERE user_id IN (#{trial})",
          "DELETE FROM accounts_preferences WHERE user_id IN (#{trial})",
          "DELETE FROM app_communities WHERE id IN
             (SELECT community_id FROM app_users_communities WHERE user_id IN (#{trial}))",
          "DELETE FROM app_users WHERE id IN (#{trial})"
        ] do
      Repo.query!(statement, [id])
    end

    Repo.query!("DELETE FROM accounts_users WHERE id = $1 AND trial_expires_at IS NOT NULL", [id])
    :ok
  end

  @doc """
  Deletes outright the trial users whose trial ended before the time,
  with everything of theirs: their events and preferences, the
  communities they are in, which are theirs alone, their user's row and
  with it their memberships, and their account with its tokens. How
  many users.
  """
  def delete_expired_before(time) do
    expired = "SELECT id FROM accounts_users WHERE trial_expires_at < $1"

    for statement <- [
          "DELETE FROM app_events WHERE user_id IN (#{expired})",
          "DELETE FROM accounts_preferences WHERE user_id IN (#{expired})",
          "DELETE FROM app_communities WHERE id IN
             (SELECT community_id FROM app_users_communities WHERE user_id IN (#{expired}))",
          "DELETE FROM app_users WHERE id IN (#{expired})"
        ] do
      Repo.query!(statement, [time])
    end

    %{num_rows: count} = Repo.query!("DELETE FROM accounts_users WHERE trial_expires_at < $1", [time])
    count
  end
end
