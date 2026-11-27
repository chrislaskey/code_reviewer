defmodule Core.Data.AppUsersCommunities do
  @moduledoc "Membership records: the `app_users_communities` schema and its reads and writes."
  use Ecto.Schema
  import Core.Data.SyncedRow, only: [sync_columns: 0]
  import Ecto.Query
  alias Core.Data.SyncedRow
  alias Core.Repo

  @table "app_users_communities"
  @role_member "member"
  @primary_key {:id, Ecto.UUID, autogenerate: false}
  schema @table do
    field :user_id, Ecto.UUID
    field :community_id, Ecto.UUID
    field :role, :string, default: @role_member
    sync_columns()
  end

  def table, do: @table
  def role_member, do: @role_member

  @doc "The row as the wire carries it."
  def to_wire(membership) do
    Map.merge(SyncedRow.wire(membership), %{
      "user_id" => membership.user_id,
      "community_id" => membership.community_id,
      "role" => membership.role
    })
  end

  @doc "Gets the user's row for the community, deleted or not: there is one at most."
  def get_by_user_and_community(user_id, community_id),
    do: Repo.get_by(__MODULE__, user_id: user_id, community_id: community_id)

  @doc "Gets the user's live membership of the community, or nil."
  def get_live(user_id, community_id) do
    Repo.one(
      from m in __MODULE__,
        where: m.user_id == ^user_id and m.community_id == ^community_id and is_nil(m.deleted_at)
    )
  end

  @doc "Lists the user's live memberships."
  def list_live_of_user(user_id),
    do: Repo.all(from m in __MODULE__, where: m.user_id == ^user_id and is_nil(m.deleted_at))

  @doc "Inserts a new row, or writes over its stored one, and publishes it."
  def create(membership), do: SyncedRow.write(membership, @table)
  def update(membership), do: SyncedRow.write(membership, @table)

  @doc "The rows the user sees with a version in the range: their own, and every row of a community they are a live member of."
  def list_synced_after(user_id, after_version, through, limit) do
    Repo.all(
      from m in __MODULE__,
        where: m.global_sync_version > ^after_version and m.global_sync_version <= ^through,
        where:
          m.user_id == ^user_id or
            fragment(
              "? IN (SELECT community_id FROM app_users_communities WHERE user_id = ? AND deleted_at IS NULL)",
              m.community_id,
              type(^user_id, Ecto.UUID)
            ),
        order_by: m.global_sync_version,
        limit: ^limit
    )
  end
end
