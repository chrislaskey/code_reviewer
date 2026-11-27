defmodule Core.App.Memberships do
  @moduledoc "Joining and leaving a community, as `memberships::create` and `delete` do."
  alias Core.Data.{AppCommunities, AppSyncVersions, AppUsersCommunities, SyncedRow}
  alias Core.Helpers.{Ids, Time}
  alias Core.Repo

  @doc "Joins a community the actor sees, or revives a membership left before. It is published."
  def create(actor_id, community_id) do
    Repo.transaction(fn ->
      AppSyncVersions.lock()

      with %AppCommunities{deleted_at: nil} <- AppCommunities.get(community_id) || Repo.rollback(:not_found) do
        now = Time.now()

        membership =
          case AppUsersCommunities.get_by_user_and_community(actor_id, community_id) do
            nil ->
              AppUsersCommunities.create(%AppUsersCommunities{
                id: Ids.new(),
                user_id: actor_id,
                community_id: community_id,
                inserted_at: now,
                updated_at: now,
                version: 1,
                last_synced_at: now
              })

            %AppUsersCommunities{deleted_at: nil} = live ->
              live

            left ->
              AppUsersCommunities.update(%{SyncedRow.changed(left, now) | deleted_at: nil, deleted_by_id: nil})
          end

        AppUsersCommunities.to_wire(membership)
      else
        _ -> Repo.rollback(:not_found)
      end
    end)
  end

  @doc "Leaves a community: soft-deletes the actor's live membership of it, and publishes it."
  def delete(actor_id, community_id) do
    Repo.transaction(fn ->
      AppSyncVersions.lock()

      case AppUsersCommunities.get_live(actor_id, community_id) do
        nil ->
          Repo.rollback(:not_found)

        membership ->
          now = Time.now()
          AppUsersCommunities.update(%{SyncedRow.changed(membership, now) | deleted_at: now, deleted_by_id: actor_id})
          %{}
      end
    end)
  end
end
