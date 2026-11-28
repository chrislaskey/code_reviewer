defmodule Core.Data.AccountsPreferences do
  @moduledoc """
  Preference records: the `accounts_preferences` schema and its reads and
  writes, one row per user and key, its value any JSON. The one accounts
  table that syncs, to its own user's devices only.
  """
  use Ecto.Schema
  import Core.Data.SyncedRow, only: [sync_columns: 0]
  import Ecto.Query
  alias Core.Data.{AppSyncChanges, SyncedRow}
  alias Core.Repo

  @table "accounts_preferences"
  @primary_key {:id, Ecto.UUID, autogenerate: false}
  schema @table do
    field :user_id, Ecto.UUID
    field :key, :string
    field :value, Core.Helpers.Json
    sync_columns()
  end

  def table, do: @table

  @doc "The row as the wire carries it."
  def to_wire(preference) do
    Map.merge(SyncedRow.wire(preference), %{
      "user_id" => preference.user_id,
      "key" => preference.key,
      "value" => preference.value
    })
  end

  @doc "Gets a row by id, deleted or not."
  def get(id), do: Repo.get(__MODULE__, id)

  @doc "Gets the user's live row of the key, or nil."
  def get_by_user_and_key(user_id, key) do
    Repo.one(from p in __MODULE__, where: p.user_id == ^user_id and p.key == ^key and is_nil(p.deleted_at))
  end

  @doc "Lists the user's live rows."
  def list_of_user(user_id), do: Repo.all(from p in __MODULE__, where: p.user_id == ^user_id and is_nil(p.deleted_at))

  @doc """
  Inserts a new row, or writes over its stored one, and publishes it. In
  SQL, so a null value is JSON's `null`, as Rust writes it, not SQL's
  NULL, which Ecto would write.
  """
  def create(preference), do: write(preference)
  def update(preference), do: write(preference)

  defp write(preference) do
    preference = %{preference | global_sync_version: AppSyncChanges.record(@table, preference.id)}

    Repo.query!(
      """
      INSERT INTO accounts_preferences
        (id, user_id, key, value, inserted_at, updated_at, version, global_sync_version, deleted_at, deleted_by_id)
      VALUES ($1, $2, $3, $4::text::jsonb, $5, $6, $7, $8, $9, $10)
      ON CONFLICT (id) DO UPDATE SET value = EXCLUDED.value, updated_at = EXCLUDED.updated_at,
        version = EXCLUDED.version, global_sync_version = EXCLUDED.global_sync_version,
        deleted_at = EXCLUDED.deleted_at, deleted_by_id = EXCLUDED.deleted_by_id
      """,
      [
        Ecto.UUID.dump!(preference.id),
        Ecto.UUID.dump!(preference.user_id),
        preference.key,
        Jason.encode!(preference.value),
        preference.inserted_at,
        preference.updated_at,
        preference.version,
        preference.global_sync_version,
        preference.deleted_at,
        preference.deleted_by_id && Ecto.UUID.dump!(preference.deleted_by_id)
      ]
    )

    preference
  end

  @doc "Deletes every row of the user outright, with their account: no device of anyone else's has one."
  def delete_of_user(user_id), do: Repo.delete_all(from p in __MODULE__, where: p.user_id == ^user_id)

  @doc "The user's own rows with a version in the range: nobody else sees them."
  def list_synced_after(user_id, after_version, through, limit) do
    Repo.all(
      from p in __MODULE__,
        where: p.global_sync_version > ^after_version and p.global_sync_version <= ^through,
        where: p.user_id == ^user_id,
        order_by: p.global_sync_version,
        limit: ^limit
    )
  end
end
