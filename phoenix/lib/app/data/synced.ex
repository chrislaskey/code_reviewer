defmodule App.Data.Synced do
  @moduledoc """
  Marks a schema as synced: its table exists on every client with the
  same columns, and its rows travel through `App.Data.Sync`. A synced
  table carries the sync columns of `docs/design/offline-sync.md`:
  `version`, `updated_at`, `last_synced_at`, `deleted_at` and
  `deleted_by_id`.

      use App.Data.Synced

  The mark is a persisted module attribute, so tools read it from the
  compiled module with `synced?/1`. The ordered list of synced schemas,
  parents first, is `App.Data.Sync.schemas/0`.
  """

  defmacro __using__(_opts) do
    quote do
      Module.register_attribute(__MODULE__, :synced, persist: true)
      @synced true
    end
  end

  @doc """
  Sets `deleted_by_id` to match a `deleted_at` change: the user who
  deleted, or nil when the row comes back. For the `event_changeset/3` of
  a resource that changes through events.
  """
  def put_deleted_by(changeset, user_id) do
    case Ecto.Changeset.fetch_change(changeset, :deleted_at) do
      {:ok, nil} -> Ecto.Changeset.put_change(changeset, :deleted_by_id, nil)
      {:ok, _at} -> Ecto.Changeset.put_change(changeset, :deleted_by_id, user_id)
      :error -> changeset
    end
  end

  @doc """
  Whether the schema module is marked as synced.
  """
  def synced?(module) when is_atom(module) do
    Keyword.get(module.__info__(:attributes), :synced) == [true]
  end
end
