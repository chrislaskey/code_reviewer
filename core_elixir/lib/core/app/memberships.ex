defmodule Core.App.Memberships do
  @moduledoc """
  Joining and leaving a community, as `memberships::create` and `delete`
  do: each published, and recorded as the actor's event. A trial user and
  anyone else are never in one community.
  """
  alias Core.Data.{AppCommunities, AppSyncVersions, AppUsersCommunities, SyncedRow, TrialUsers}
  alias Core.Helpers.{Ids, Time}
  alias Core.Repo
  alias Core.Sync.{Answers, Events}

  @doc "Joins a community the actor sees, or revives a membership left before."
  def create(actor_id, community_id) do
    Repo.transaction(fn ->
      AppSyncVersions.lock()
      actor_id |> create_in(community_id) |> AppUsersCommunities.to_wire()
    end)
  end

  @doc "Joins the community in the caller's transaction, which it rolls back with `:not_found` for one the actor may not join."
  def create_in(actor_id, community_id) do
    case AppCommunities.get(community_id) do
      %AppCommunities{deleted_at: nil} -> :ok
      _ -> Repo.rollback(:not_found)
    end

    if TrialUsers.join_is_walled_off?(actor_id, community_id), do: Repo.rollback(:not_found)
    table = AppUsersCommunities.table()
    now = Time.now()

    case AppUsersCommunities.get_by_user_and_community(actor_id, community_id) do
      nil ->
        membership =
          AppUsersCommunities.create(%AppUsersCommunities{
            id: Ids.new(),
            user_id: actor_id,
            community_id: community_id,
            inserted_at: now,
            updated_at: now,
            version: 1
          })

        changes = %{"user_id" => actor_id, "community_id" => community_id, "role" => membership.role}
        recorded(actor_id, Events.create(table, membership.id, changes), membership)

      %AppUsersCommunities{deleted_at: nil} = live ->
        live

      left ->
        base = %{"deleted_at" => Time.to_wire(left.deleted_at)}
        event = Events.update(table, left.id, left.version, base, %{"deleted_at" => nil})
        membership = AppUsersCommunities.update(%{SyncedRow.changed(left, now) | deleted_at: nil, deleted_by_id: nil})
        recorded(actor_id, event, membership)
    end
  end

  @doc "Leaves a community: soft-deletes the actor's live membership of it."
  def delete(actor_id, community_id) do
    Repo.transaction(fn ->
      AppSyncVersions.lock()

      case AppUsersCommunities.get_live(actor_id, community_id) do
        nil ->
          Repo.rollback(:not_found)

        membership ->
          now = Time.now()

          event =
            Events.update(AppUsersCommunities.table(), membership.id, membership.version, %{"deleted_at" => nil}, %{
              "deleted_at" => Time.to_wire(now)
            })

          left =
            AppUsersCommunities.update(%{SyncedRow.changed(membership, now) | deleted_at: now, deleted_by_id: actor_id})

          recorded(actor_id, event, left)
          %{}
      end
    end)
  end

  defp recorded(actor_id, event, membership) do
    Events.record(actor_id, event, Answers.applied(event["id"], membership.version, %{}, nil))
    membership
  end
end
