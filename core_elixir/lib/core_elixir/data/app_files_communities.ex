defmodule CoreElixir.Data.AppFilesCommunities do
  @moduledoc """
  File community records: the `app_files_communities` schema and its
  reads and writes. Only the files projector writes them.
  """
  use Ecto.Schema
  import CoreElixir.Data.SyncedRow, only: [sync_columns: 0]
  import Ecto.Query
  alias CoreElixir.Data.SyncedRow
  alias CoreElixir.Repo

  @table "app_files_communities"
  @primary_key {:id, Ecto.UUID, autogenerate: false}
  schema @table do
    field :file_id, Ecto.UUID
    field :community_id, Ecto.UUID
    field :created_by_id, Ecto.UUID
    sync_columns()
  end

  def table, do: @table

  @doc "The row as the wire carries it."
  def to_wire(row) do
    Map.merge(SyncedRow.wire(row), %{
      "file_id" => row.file_id,
      "community_id" => row.community_id,
      "created_by_id" => row.created_by_id
    })
  end

  @doc "Gets a row by id, deleted or not."
  def get(id), do: Repo.get(__MODULE__, id)

  @doc "Gets the file's row for the community, deleted or not: there is one at most, revived by a put back."
  def get_by_file_and_community(file_id, community_id),
    do: Repo.get_by(__MODULE__, file_id: file_id, community_id: community_id)

  @doc "Lists the live rows of the files in communities the user is a live member of."
  def list_visible_of_files(_user_id, []), do: []

  def list_visible_of_files(user_id, file_ids) do
    Repo.all(
      from r in __MODULE__,
        where: r.file_id in ^file_ids and is_nil(r.deleted_at),
        where: ^visible_to(user_id)
    )
  end

  @doc "Inserts a new row, or writes over its stored one, and publishes it."
  def create(row), do: SyncedRow.write(row, @table)
  def update(row), do: SyncedRow.write(row, @table)

  @doc "The rows the user sees with a version in the range."
  def list_synced_after(user_id, after_version, through, limit) do
    Repo.all(
      from r in __MODULE__,
        where: r.global_sync_version > ^after_version and r.global_sync_version <= ^through,
        where: ^visible_to(user_id),
        order_by: r.global_sync_version,
        limit: ^limit
    )
  end

  @doc "The live rows of the community with a version in the range, for a backfill."
  def list_live_of_community_after(community_id, after_version, through, limit) do
    Repo.all(
      from r in __MODULE__,
        where: r.community_id == ^community_id and is_nil(r.deleted_at),
        where: r.global_sync_version > ^after_version and r.global_sync_version <= ^through,
        order_by: r.global_sync_version,
        limit: ^limit
    )
  end

  # Every row of a community the user is a live member of
  defp visible_to(user_id) do
    dynamic(
      [r],
      fragment(
        "? IN (SELECT community_id FROM app_users_communities WHERE user_id = ? AND deleted_at IS NULL)",
        r.community_id,
        type(^user_id, Ecto.UUID)
      )
    )
  end
end
