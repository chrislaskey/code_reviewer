defmodule Core.Data.AppUsers do
  @moduledoc "User records: the `app_users` schema and its reads and writes."
  use Ecto.Schema
  import Core.Data.SyncedRow, only: [sync_columns: 0]
  import Ecto.Query
  alias Core.Data.SyncedRow
  alias Core.Repo

  @table "app_users"
  @max_name_length 80
  @primary_key {:id, Ecto.UUID, autogenerate: false}
  schema @table do
    field :name, :string
    field :avatar_url, :string
    sync_columns()
  end

  def table, do: @table
  def max_name_length, do: @max_name_length

  @doc "The row as the wire carries it."
  def to_wire(user),
    do: Map.merge(SyncedRow.wire(user), %{"name" => user.name, "avatar_url" => user.avatar_url})

  @doc "Gets a row by id, deleted or not."
  def get(id), do: Repo.get(__MODULE__, id)

  @doc "Lists the rows with the ids, deleted or not."
  def list_by_ids([]), do: []
  def list_by_ids(ids), do: Repo.all(from u in __MODULE__, where: u.id in ^ids)

  @doc "Every row's picture, deleted rows' too."
  def list_avatar_urls,
    do: Repo.all(from u in __MODULE__, where: not is_nil(u.avatar_url), select: u.avatar_url)

  @doc "Whether a live user other than `except_id` has the name, in any case."
  def exists_by_name?(name, except_id) do
    Repo.exists?(
      from u in __MODULE__,
        where: fragment("lower(?) = lower(?)", u.name, ^name) and u.id != ^except_id and is_nil(u.deleted_at)
    )
  end

  @doc "Inserts a new row, or writes over its stored one, and publishes it."
  def create(user), do: SyncedRow.write(user, @table)
  def update(user), do: SyncedRow.write(user, @table)

  @doc "The rows the user sees with a version in the range: themselves, and every live member of a community they are a live member of."
  def list_synced_after(user_id, after_version, through, limit) do
    Repo.all(
      from u in __MODULE__,
        where: u.global_sync_version > ^after_version and u.global_sync_version <= ^through,
        where:
          u.id == type(^user_id, Ecto.UUID) or
            fragment(
              "? IN (SELECT uc.user_id FROM app_users_communities uc WHERE uc.deleted_at IS NULL AND uc.community_id IN (SELECT community_id FROM app_users_communities WHERE user_id = ? AND deleted_at IS NULL))",
              u.id,
              type(^user_id, Ecto.UUID)
            ),
        order_by: u.global_sync_version,
        limit: ^limit
    )
  end
end
