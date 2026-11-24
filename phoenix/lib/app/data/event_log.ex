defmodule App.Data.EventLog do
  @moduledoc """
  The shared event log, `app_events` over `App.Data.Events`: which
  resources keep one, and recording a change in its stream. See "The
  event log" in `docs/design/related-resources.md`.

  A resource opts in by defining `event_stream/1` on its schema, which
  names the stream a row's events go in: `{"app_posts", post_id}` for a
  post and the rows that are part of it. A resource without it, such as
  a profile, still changes through events, but nothing is stored.
  """

  import Ecto.Query, only: [from: 2]

  alias App.Data.Event
  alias App.Data.Events
  alias App.Data.Sync.Merge

  @doc """
  Whether the schema's rows keep their events.
  """
  def logged?(schema) do
    Code.ensure_loaded?(schema) and function_exported?(schema, :event_stream, 1)
  end

  @doc """
  The events of a stream, in order.
  """
  def stream(table, id), do: Events.list(filters: [stream_table: table, stream_id: id])

  @doc """
  Records a change to `row`, at the row's version, next in its stream.
  `attrs` has the rest: `user_id`, `type`, `base_version`, `changes`,
  `conflicts`, and `id` and the `caused_by_*` fields when there are.
  Returns `{:ok, nil}` for a row whose resource keeps no log.
  """
  def record(%schema{} = row, attrs) do
    if logged?(schema) do
      {stream_table, stream_id} = schema.event_stream(row)

      attrs
      |> Map.merge(%{
        stream_table: stream_table,
        stream_id: stream_id,
        sequence: next_sequence(stream_table, stream_id),
        row_table: schema.__schema__(:source),
        row_id: row.id,
        version: row.version,
        changes: wire(attrs.changes),
        conflicts: Map.get(attrs, :conflicts, %{})
      })
      |> Events.create()
    else
      {:ok, nil}
    end
  end

  defp next_sequence(table, id) do
    last =
      App.Repo.one(
        from(e in Event, where: e.stream_table == ^table and e.stream_id == ^id, select: max(e.sequence))
      )

    (last || 0) + 1
  end

  defp wire(changes), do: Map.new(changes, fn {field, value} -> {to_string(field), Merge.wire(value)} end)

  @doc """
  Removes the streams whose own row is gone, after the purge. Returns how
  many events went.
  """
  def purge_streams(streams) do
    Enum.reduce(streams, 0, fn {table, schema}, count ->
      {gone, _} =
        App.Repo.delete_all(
          from(e in Event,
            where: e.stream_table == ^table and e.stream_id not in subquery(from(r in schema, select: r.id))
          )
        )

      count + gone
    end)
  end
end
