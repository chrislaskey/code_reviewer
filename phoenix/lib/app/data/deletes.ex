defmodule App.Data.Deletes do
  @moduledoc """
  Copies a delete onto the rows below, so a row is never live while a
  row above it is deleted. See "Deletes" in
  `docs/design/related-resources.md`.

  A schema names the rows above it with `parents/0`, by foreign key:

      def parents, do: [post_id: App.Data.Post, tag_id: App.Data.Tag]

  and the rows below a row are those of every synced schema naming it.

  - **Deleting** a row sets its `deleted_at` and `deleted_by_id` on every
    live row below it.
  - **Undeleting** it revives the rows below whose `deleted_at` is the one
    cleared, so a row deleted on its own, at another moment, stays
    deleted. A row with another parent still deleted takes that parent's
    `deleted_at` instead, so undeleting that parent later finds it.
  - **Writing under a deleted parent** keeps the row, deleted with the
    parent, see `inherit/1`.

  Each copy increments the row's version, sets `last_synced_at` so it
  syncs, and is recorded in its stream when the row's resource keeps a
  log, caused by the parent's change, see `App.Data.EventLog`.
  """

  import Ecto.Query, only: [from: 2]

  alias App.Data.EventLog

  @doc """
  Copies the change of `row`'s `deleted_at`, from `before`, onto the rows
  below it, as `user_id`'s doing. Returns `:ok`.
  """
  def copy_down(%schema{deleted_at: at} = _before, %schema{deleted_at: at}, _user_id), do: :ok

  def copy_down(%schema{} = before, %schema{} = row, user_id) do
    for {child, key} <- children(schema), below <- below(child, key, row.id, before.deleted_at) do
      copy_to(below, row, user_id)
    end

    :ok
  end

  # The rows below `id` in `child` with the `deleted_at` the parent had
  defp below(child, key, id, nil),
    do: App.Repo.all(from(r in child, where: field(r, ^key) == ^id and is_nil(r.deleted_at)))

  defp below(child, key, id, at),
    do: App.Repo.all(from(r in child, where: field(r, ^key) == ^id and r.deleted_at == ^at))

  defp copy_to(%schema{} = below, parent, user_id) do
    {deleted_at, deleted_by_id} =
      case parent do
        %{deleted_at: nil} -> deleted_parent(below)
        %{deleted_at: at, deleted_by_id: by} -> {at, by}
      end

    App.Repo.update_all(from(r in schema, where: r.id == ^below.id),
      set: [deleted_at: deleted_at, deleted_by_id: deleted_by_id, last_synced_at: DateTime.utc_now()],
      inc: [version: 1]
    )

    copied = App.Repo.get!(schema, below.id)

    {:ok, _event} =
      EventLog.record(copied, %{
        user_id: user_id,
        type: :update,
        base_version: below.version,
        changes: %{deleted_at: deleted_at},
        caused_by_table: parent.__struct__.__schema__(:source),
        caused_by_id: parent.id,
        caused_by_version: parent.version
      })

    copy_down(below, copied, user_id)
  end

  @doc """
  `{deleted_at, deleted_by_id}` of a deleted row above `row`, or
  `{nil, nil}` when every one is live.
  """
  def deleted_parent(%schema{} = row) do
    parents =
      if Code.ensure_loaded?(schema) and function_exported?(schema, :parents, 0),
        do: schema.parents(),
        else: []

    Enum.find_value(parents, {nil, nil}, fn {key, parent} ->
      case Map.fetch!(row, key) && App.Repo.get(parent, Map.fetch!(row, key)) do
        %{deleted_at: %DateTime{} = at, deleted_by_id: by} -> {at, by}
        _ -> nil
      end
    end)
  end

  @doc """
  A new row, or a revived one, under a deleted parent stays deleted with
  it: the changeset gets the parent's `deleted_at`, so undeleting the
  parent brings the row back.
  """
  def inherit(%Ecto.Changeset{} = changeset) do
    case Ecto.Changeset.get_field(changeset, :deleted_at) do
      nil ->
        case deleted_parent(Ecto.Changeset.apply_changes(changeset)) do
          {nil, nil} -> changeset
          {at, by} -> Ecto.Changeset.change(changeset, deleted_at: at, deleted_by_id: by)
        end

      _deleted ->
        changeset
    end
  end

  # The schemas whose rows sit below `schema`'s, with their foreign key
  defp children(schema) do
    for child <- App.Data.Sync.schemas(),
        Code.ensure_loaded?(child) and function_exported?(child, :parents, 0),
        {key, ^schema} <- child.parents(),
        do: {child, key}
  end
end
