defmodule App.Data.EventFlow do
  @moduledoc """
  Applies an event to a row of a resource that changes through events,
  the same way for every such resource, see "Events" in
  `docs/design/offline-sync.md` and "Writing" in
  `docs/design/related-resources.md`. A resource's flow describes it
  with a spec and calls `apply/3`, such as `App.Data.Posting` for posts
  and links.

  The spec is a map:

    * `:schema` and `:context`
    * `:fields`: `%{create: [...], update: [...]}`, the fields an event
      may set, as strings
    * `:unknown`: the reason for an update to a row the server lacks
    * `:check_create`: `fn scope, changes -> {:ok, changeset_options} |
      {:error, reason, server_row_or_nil} end`
    * `:check_update`: `fn scope, row, changes -> :ok | {:error, reason} end`,
      with the changes the event asks for
    * `:normalize`, optional: `fn changes -> changes end`, run before the
      changeset, such as trimming a tag's name

  In order: a retried event of a logged resource gets its recorded
  answer; a field the event may not set refuses it; a create makes the
  row, under a deleted parent deleted with it; an update merges field by
  field, records the event, and copies a change of `deleted_at` onto the
  rows below, see `App.Data.Deletes`.

  Returns `{:ok, answer, row}`, the answer being the recorded
  `App.Data.Event`, or `%{version: ..., conflicts: ...}` for a resource
  without a log; `{:error, reason, row_or_nil}`; or
  `{:error, changeset, row_or_nil}` for a row the changeset refused.
  """

  alias App.Data.Deletes
  alias App.Data.Event
  alias App.Data.EventLog
  alias App.Data.Events
  alias App.Data.Sync.Merge
  alias App.Web.Scope

  def apply(%Scope{user: user} = scope, %{id: id} = event, spec) do
    case EventLog.logged?(spec.schema) && Events.get(id) do
      %Event{user_id: user_id} = recorded when user_id == user.id ->
        {:ok, recorded, current(spec, recorded.row_id)}

      %Event{} ->
        {:error, "event_id_taken", nil}

      _new ->
        apply_new(scope, event, spec)
    end
  end

  defp apply_new(scope, %{type: type, changes: changes} = event, spec) do
    case Map.keys(changes) -- Map.fetch!(spec.fields, type) do
      [] ->
        normalize = Map.get(spec, :normalize, & &1)
        apply_checked(scope, %{event | changes: normalize.(changes)}, spec)

      fields ->
        {:error, "not_editable: " <> Enum.join(Enum.sort(fields), ", "), current(spec, event.row_id)}
    end
  end

  defp apply_checked(%Scope{user: user} = scope, %{type: :create} = event, spec) do
    with nil <- current(spec, event.row_id),
         {:ok, options} <- spec.check_create.(scope, event.changes) do
      changeset =
        spec.schema
        |> struct(id: event.row_id)
        |> spec.context.change(event.changes, options)
        |> Deletes.inherit()

      with {:ok, row} <- insert(changeset),
           {:ok, answer} <- record(row, user, :create, 0, event.changes, %{}, event.id) do
        {:ok, answer, row}
      end
    else
      %_{} = row -> {:error, "exists", row}
      {:error, reason, row} -> {:error, reason, row}
    end
  end

  defp apply_checked(%Scope{user: user} = scope, %{type: :update} = event, spec) do
    with %_{} = row <- current(spec, event.row_id),
         :ok <- spec.check_update.(scope, row, event.changes) do
      values = Merge.values(row, spec.fields.update)

      {applied, conflicts} =
        Merge.merge(values, row.version, event.base_version, event.base, event.changes)

      applied = keep_under_deleted_parent(row, applied)

      with {:ok, updated} <- update(spec, row, applied, user.id),
           {:ok, answer} <- record(updated, user, :update, event.base_version, applied, conflicts, event.id),
           :ok <- Deletes.copy_down(row, updated, user.id) do
        {:ok, answer, updated}
      end
    else
      nil -> {:error, spec.unknown, nil}
      {:error, reason} -> {:error, reason, current(spec, event.row_id)}
    end
  end

  defp current(spec, id), do: spec.context.get(id, include_deleted: true)

  # Undeleting a row while a row above it is still deleted keeps it
  # deleted with that one
  defp keep_under_deleted_parent(row, %{"deleted_at" => nil} = applied) do
    case Deletes.deleted_parent(row) do
      {nil, nil} -> applied
      {at, _by} -> Map.put(applied, "deleted_at", Merge.wire(at))
    end
  end

  defp keep_under_deleted_parent(_row, applied), do: applied

  defp insert(changeset) do
    case App.Repo.insert(changeset) do
      {:ok, row} -> {:ok, row}
      {:error, changeset} -> {:error, changeset, nil}
    end
  end

  defp update(_spec, row, applied, _user_id) when applied == %{}, do: {:ok, row}

  defp update(spec, row, applied, user_id) do
    case spec.context.update(row, applied,
           changesets: fn row, params, options -> spec.schema.event_changeset(row, params, options) end,
           deleted_by_id: user_id
         ) do
      {:ok, row} -> {:ok, row}
      {:error, changeset} -> {:error, changeset, row}
    end
  end

  defp record(row, user, type, base_version, changes, conflicts, id) do
    case EventLog.record(row, %{
           id: id,
           user_id: user.id,
           type: type,
           base_version: base_version,
           changes: changes,
           conflicts: conflicts
         }) do
      {:ok, nil} -> {:ok, %{version: row.version, conflicts: conflicts}}
      {:ok, event} -> {:ok, event}
      {:error, changeset} -> {:error, changeset, row}
    end
  end
end
