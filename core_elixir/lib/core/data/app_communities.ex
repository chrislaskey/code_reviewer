defmodule Core.Data.AppCommunities do
  @moduledoc "Community records: the `app_communities` schema and its reads and writes."
  use Ecto.Schema
  import Core.Data.SyncedRow, only: [sync_columns: 0]
  import Ecto.Query
  alias Core.Data.SyncedRow
  alias Core.Repo

  @table "app_communities"
  @primary_key {:id, Ecto.UUID, autogenerate: false}
  schema @table do
    field :name, :string
    sync_columns()
  end

  def table, do: @table

  @doc "The row as the wire carries it."
  def to_wire(community), do: Map.put(SyncedRow.wire(community), "name", community.name)

  @doc "Gets a row by id, deleted or not."
  def get(id), do: Repo.get(__MODULE__, id)

  @doc "Lists the rows with the ids, deleted or not, by name."
  def list_by_ids([]), do: []
  def list_by_ids(ids), do: Repo.all(from c in __MODULE__, where: c.id in ^ids, order_by: [c.name, c.id])

  @doc "Inserts a new row and publishes it."
  def create(community), do: SyncedRow.write(community, @table)

  @doc "Lists the live communities the user sees, by name."
  def list_live(user_id) do
    Repo.all(
      from c in __MODULE__,
        where: is_nil(c.deleted_at),
        where: ^visible_to(user_id),
        order_by: [c.name, c.id]
    )
  end

  @doc "The rows the user sees with a version in the range."
  def list_synced_after(user_id, after_version, through, limit) do
    Repo.all(
      from c in __MODULE__,
        where: c.global_sync_version > ^after_version and c.global_sync_version <= ^through,
        where: ^visible_to(user_id),
        order_by: c.global_sync_version,
        limit: ^limit
    )
  end

  # Every community but those of another user's trial; a trial user sees
  # only their own
  defp visible_to(user_id) do
    dynamic(
      [c],
      fragment(
        "? NOT IN (SELECT uc.community_id FROM app_users_communities uc JOIN accounts_users a ON a.id = uc.user_id WHERE a.trial_expires_at IS NOT NULL AND uc.user_id != ?) AND ((SELECT trial_expires_at FROM accounts_users WHERE id = ?) IS NULL OR ? IN (SELECT community_id FROM app_users_communities WHERE user_id = ?))",
        c.id,
        type(^user_id, Ecto.UUID),
        type(^user_id, Ecto.UUID),
        c.id,
        type(^user_id, Ecto.UUID)
      )
    )
  end
end
