defmodule Core.Sync do
  @moduledoc """
  The sync protocol a device speaks through Axum, as `core/src/sync.rs`
  answers it, see `docs/design/backend-protocol.md`: push, pull and
  backfill. Cursor and row versions are decimal strings on the wire.
  """
  alias Core.Accounts.Preferences
  alias Core.App.{Files, FilesCommunities, Users}
  alias Core.Data.{AccountsPreferences, AppFiles, AppUsers, AppFilesCommunities, AppSyncVersions, AppUsersCommunities}
  alias Core.Repo
  alias Core.Sync.{Answers, Tables}

  @default_limit 200
  @max_limit 500

  @doc """
  Answers a push: applies the events in the order given, each on its
  own, and answers every one. A command is not dispatched inside a
  database transaction: its projector writes in a transaction of its own.
  """
  def push(actor_id, events, uploaded) do
    uploaded = MapSet.new(uploaded || [])

    answers =
      Enum.reduce_while(events, [], fn event, answers ->
        case apply_event(actor_id, event, MapSet.member?(uploaded, event["id"])) do
          {:error, reason} -> {:halt, {:error, reason}}
          answer -> {:cont, [answer | answers]}
        end
      end)

    case answers do
      {:error, reason} -> {:error, reason}
      answers -> {:ok, %{"events" => Enum.reverse(answers)}}
    end
  end

  # A trial's write past its limit ends the push, as in the Rust server:
  # the events before it stay applied
  defp apply_event(actor_id, event, uploaded) do
    cond do
      event["table"] == AppFiles.table() ->
        Files.apply_event(actor_id, event, uploaded)

      event["table"] == AppFilesCommunities.table() ->
        FilesCommunities.apply_event(actor_id, event)

      event["table"] == AccountsPreferences.table() ->
        with {:ok, answer} <- Preferences.apply_event(actor_id, event), do: answer

      event["table"] == AppUsers.table() ->
        with {:ok, answer} <- Users.apply_event(actor_id, event), do: answer

      true ->
        Answers.rejected(event["id"], "unknown_table")
    end
  end

  @doc "Pulls a page of one snapshot of the committed feed."
  def pull(actor_id, request) do
    snapshot(fn ->
      feed = AppSyncVersions.get()

      with {:ok, continuation} <- pull_continuation(request, feed) do
        if covered?(continuation, feed) do
          limit = limit(request)
          page = scan_page(actor_id, continuation, limit)

          cursor =
            if continuation.scan == "incremental" or is_nil(page["next"]) do
              version =
                if page["next"], do: page["next"]["range"]["after"], else: Integer.to_string(continuation.through)

              %{"feed_id" => feed.id, "global_sync_version" => version}
            end

          {:ok, Map.put(page, "cursor", cursor)}
        else
          {:ok, resync()}
        end
      end
    end)
  end

  @doc "Lists the live rows of a community the actor is in, with its own continuation."
  def backfill(actor_id, request) do
    community_id = request["community_id"]

    snapshot(fn ->
      if is_nil(AppUsersCommunities.get_live(actor_id, community_id)) do
        {:error, :not_a_member}
      else
        feed = AppSyncVersions.get()
        scan = %{"community" => community_id}

        continuation =
          case request["after"] do
            nil -> {:ok, %{feed_id: feed.id, scan: scan, after: 0, through: feed.head}}
            after_continuation -> decode_continuation(after_continuation)
          end

        with {:ok, continuation} <- continuation do
          cond do
            continuation.scan != scan ->
              {:error, :invalid_continuation}

            covered?(continuation, feed) ->
              {:ok, Map.put(scan_page(actor_id, continuation, limit(request)), "cursor", nil)}

            true ->
              {:ok, resync()}
          end
        end
      end
    end)
  end

  defp pull_continuation(%{"after" => after_continuation} = request, _feed) when not is_nil(after_continuation) do
    with {:ok, continuation} <- decode_continuation(after_continuation) do
      if request["cursor"] || match?(%{"community" => _}, continuation.scan),
        do: {:error, :invalid_continuation},
        else: {:ok, continuation}
    end
  end

  defp pull_continuation(%{"cursor" => %{"feed_id" => feed_id, "global_sync_version" => version}}, feed) do
    with {:ok, version} <- version(version) do
      # A cursor above the head resyncs through `covered?`
      {:ok, %{feed_id: feed_id, scan: "incremental", after: version, through: max(feed.head, version)}}
    end
  end

  defp pull_continuation(_request, feed),
    do: {:ok, %{feed_id: feed.id, scan: "bootstrap", after: 0, through: feed.head}}

  defp covered?(%{after: after_version, through: through} = continuation, feed) do
    required = if continuation.scan == "incremental", do: after_version, else: through
    continuation.feed_id == feed.id and through <= feed.head and after_version <= through and required >= feed.floor
  end

  defp limit(request),
    do: request |> Map.get("limit", @default_limit) |> Kernel.||(@default_limit) |> max(1) |> min(@max_limit)

  # From each table, the first rows the actor sees in the range, merged in
  # version order and cut at the limit, each after its parents
  defp scan_page(actor_id, continuation, limit) do
    rows =
      for table <- Tables.list(),
          row <- table_rows(table, actor_id, continuation, limit + 1),
          do: {row.global_sync_version, table, row}

    rows = Enum.sort_by(rows, fn {version, _table, _row} -> version end)

    {rows, next} =
      if length(rows) > limit do
        rows = Enum.take(rows, limit)
        {last, _table, _row} = List.last(rows)
        {rows, encode_continuation(%{continuation | after: last})}
      else
        {rows, nil}
      end

    {out, _sent} =
      Enum.reduce(rows, {[], MapSet.new()}, fn {_version, table, row}, {out, sent} ->
        with_parents(table, row, out, sent)
      end)

    %{"rows" => Enum.reverse(out), "next" => next, "resync" => false}
  end

  defp table_rows(table, _actor_id, %{scan: %{"community" => community_id}} = continuation, limit),
    do: Tables.list_live_in_community_after(table, community_id, continuation.after, continuation.through, limit)

  defp table_rows(table, actor_id, continuation, limit),
    do: table.list_synced_after(actor_id, continuation.after, continuation.through, limit)

  defp with_parents(table, row, out, sent) do
    key = {table.table(), row.id}

    if MapSet.member?(sent, key) do
      {out, sent}
    else
      {out, sent} =
        Enum.reduce(Tables.parents(table, row), {out, sent}, fn {parent_table, parent}, {out, sent} ->
          with_parents(parent_table, parent, out, sent)
        end)

      {[Answers.table_row(table.table(), table.to_wire(row)) | out], MapSet.put(sent, key)}
    end
  end

  defp resync, do: %{"rows" => [], "next" => nil, "cursor" => nil, "resync" => true}

  # One page is read in one snapshot
  defp snapshot(read) do
    Repo.transaction(fn ->
      Repo.query!("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ READ ONLY")

      case read.() do
        {:ok, page} -> page
        {:error, reason} -> Repo.rollback(reason)
      end
    end)
  end

  # Continuations as the Rust core writes them
  defp decode_continuation(%{
         "feed_id" => feed_id,
         "scan" => scan,
         "range" => %{"after" => after_text, "through" => through_text}
       })
       when scan in ["incremental", "bootstrap"] or (is_map(scan) and is_map_key(scan, "community")) do
    with {:ok, after_version} <- version(after_text),
         {:ok, through} <- version(through_text),
         true <- after_version <= through || {:error, :invalid_continuation} do
      {:ok, %{feed_id: feed_id, scan: scan, after: after_version, through: through}}
    end
  end

  defp decode_continuation(_), do: {:error, :invalid_continuation}

  defp encode_continuation(continuation) do
    %{
      "feed_id" => continuation.feed_id,
      "scan" => continuation.scan,
      "range" => %{
        "after" => Integer.to_string(continuation.after),
        "through" => Integer.to_string(continuation.through)
      }
    }
  end

  defp version(text) when is_binary(text) do
    case Integer.parse(text) do
      {version, ""} when version >= 0 -> {:ok, version}
      _ -> {:error, :invalid_continuation}
    end
  end

  defp version(_), do: {:error, :invalid_continuation}
end
