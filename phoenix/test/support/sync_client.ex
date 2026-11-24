defmodule App.SyncClient do
  @moduledoc """
  A simulated offline client for the sync tests: one device of one user,
  as a plain struct. It holds its rows as a client would,
  keyed by table and id, with the values as they come off the wire, and
  calls `App.Data.Sync` directly instead of over HTTP.

      phone = SyncClient.new(scope)
      phone = SyncClient.pull(phone)
      {phone, post_id} = SyncClient.create(phone, "app_posts", %{"body" => "hello"})
      phone = SyncClient.change(phone, "app_posts", post_id, %{"body" => "edited"})
      phone = SyncClient.write(phone, "app_users_communities", %{...})
      {phone, response} = SyncClient.push(phone)

  Posts, profiles, links, tags and posts_tags change through `create/3`
  and `change/4`, which queue events;
  other rows through `write/4` and `delete/4`. `conflicts` holds the
  events the server answered with conflicts, until `resolve/3`.

  `clock_offset` shifts the device's clock, in seconds, for the edits it
  makes.
  """

  alias App.Data.Sync

  defstruct scope: nil,
            cursor: nil,
            rows: %{},
            outbox: [],
            events: [],
            conflicts: [],
            clock_offset: 0,
            last_pull: []

  def new(scope, opts \\ []) do
    %__MODULE__{scope: scope, clock_offset: Keyword.get(opts, :clock_offset, 0)}
  end

  @doc """
  The client's copy of the row, or nil.
  """
  def row(%__MODULE__{rows: rows}, table, id), do: get_in(rows, [table, id])

  @doc """
  The client's rows of the table, in id order.
  """
  def rows(%__MODULE__{rows: rows}, table) do
    rows |> Map.get(table, %{}) |> Map.values() |> Enum.sort_by(& &1["id"])
  end

  @doc """
  Pulls every page from the stored cursor and applies the rows. The pages
  are kept in `last_pull` for assertions.
  """
  def pull(%__MODULE__{} = client, opts \\ []) do
    pull_pages(%{client | last_pull: []}, nil, Keyword.get(opts, :limit, 100))
  end

  defp pull_pages(client, after_token, limit) do
    params = %{"cursor" => client.cursor, "after" => after_token, "limit" => limit}
    {:ok, response} = Sync.pull(client.scope, params)
    response = wire(response)
    client = %{client | last_pull: client.last_pull ++ [response]}

    if response["resync"] do
      # The server purged tombstones this device never saw: start over
      pull_pages(%{client | rows: %{}, cursor: nil}, nil, limit)
    else
      client = Enum.reduce(response["rows"], client, &apply_server_row(&2, &1))

      case response["next"] do
        nil -> %{client | cursor: response["cursor"]}
        token -> pull_pages(client, token, limit)
      end
    end
  end

  @doc """
  Backfills a community the user joined, applying every page.
  """
  def backfill(%__MODULE__{} = client, community_id, opts \\ []) do
    backfill_pages(%{client | last_pull: []}, community_id, nil, Keyword.get(opts, :limit, 100))
  end

  defp backfill_pages(client, community_id, after_token, limit) do
    params = %{"after" => after_token, "limit" => limit}
    {:ok, response} = Sync.backfill(client.scope, community_id, params)
    response = wire(response)
    client = %{client | last_pull: client.last_pull ++ [response]}
    client = Enum.reduce(response["rows"], client, &apply_server_row(&2, &1))

    case response["next"] do
      nil -> client
      token -> backfill_pages(client, community_id, token, limit)
    end
  end

  @doc """
  A local write of a server-wins row: upserts the row with
  a client-generated id and `updated_at` from the device clock, and
  queues it for the next push. The columns not given keep the client's
  current values, or nil. A new row has version 0, not yet on the server;
  the client never changes a version itself.
  """
  def write(%__MODULE__{} = client, table, columns, opts \\ []) do
    id = columns["id"] || Ecto.UUID.generate(version: 7)
    now = device_time(client, Keyword.get(opts, :at))

    row =
      client
      |> local_row(table, id, now)
      |> Map.merge(columns)
      |> Map.put("updated_at", now)

    client
    |> put_row(table, row)
    |> Map.update!(:outbox, &Enum.uniq(&1 ++ [{table, id}]))
  end

  @doc """
  A local soft delete, queued like a write.
  """
  def delete(%__MODULE__{scope: scope} = client, table, id, opts \\ []) do
    now = device_time(client, Keyword.get(opts, :at))
    write(client, table, %{"id" => id, "deleted_at" => now, "deleted_by_id" => scope.user.id}, opts)
  end

  @doc """
  A new row of an event table, by the user: written locally at
  version 0 and queued as a `create` event. Returns the client and the
  row's id.
  """
  def create(%__MODULE__{scope: scope} = client, table, changes, opts \\ []) do
    id = Ecto.UUID.generate(version: 7)
    now = device_time(client, Keyword.get(opts, :at))

    row =
      client
      |> local_row(table, id, now)
      |> Map.merge(changes)
      |> put_owner(table, scope.user.id)

    event = new_event(table, id, "create", 0, %{}, changes)
    {client |> put_row(table, row) |> queue_event(event), id}
  end

  @doc """
  A change to a row of an event table: the row changes at once,
  and the event records the version and the values the change started
  from, as `base`, and the new values.
  """
  def change(%__MODULE__{} = client, table, id, changes, opts \\ []) do
    now = device_time(client, Keyword.get(opts, :at))
    current = row(client, table, id)
    base = Map.take(current, Map.keys(changes))
    event = new_event(table, id, "update", current["version"], base, changes)

    client
    |> put_row(table, current |> Map.merge(changes) |> Map.put("updated_at", now))
    |> queue_event(event)
  end

  @doc """
  Resolves a conflicted event: `:ignore` lets the client's values go,
  `:reapply` queues them again as a new event, starting from the row as
  the server has it now.
  """
  def resolve(%__MODULE__{} = client, event_id, how) when how in [:ignore, :reapply] do
    {[conflict], rest} = Enum.split_with(client.conflicts, &(&1["id"] == event_id))
    client = %{client | conflicts: rest}

    case how do
      :ignore ->
        client

      :reapply ->
        yours = Map.new(conflict["conflicts"], fn {field, %{"yours" => value}} -> {field, value} end)
        change(client, conflict["table"], conflict["row_id"], yours)
    end
  end

  @doc """
  Pushes the outbox and the events, and applies the response. An
  accepted row gets its `last_synced_at` and `version`; a rejected row is
  replaced by the server's version, or dropped when the server has none.
  An answered event leaves the queue, into `conflicts` when it has any,
  and the server's row replaces the client's once no event for it is
  still queued. Returns the client and the response.
  """
  def push(%__MODULE__{} = client) do
    rows = for {table, id} <- client.outbox, do: %{"table" => table, "row" => row(client, table, id)}
    events = Enum.map(client.events, &Map.delete(&1, "conflicts"))
    sent = client.events
    response = wire(Sync.push(client.scope, rows, events))
    client = %{client | outbox: []}

    client =
      Enum.reduce(response["accepted"], client, fn %{"table" => table, "id" => id} = accepted, client ->
        row = Map.merge(row(client, table, id), Map.take(accepted, ["last_synced_at", "version"]))
        put_row(client, table, row)
      end)

    client =
      Enum.reduce(response["rejected"], client, fn %{"table" => table, "id" => id, "server" => server},
                                                   client ->
        client = drop_row(client, table, id)
        if server, do: apply_server_row(client, server), else: client
      end)

    client =
      Enum.reduce(response["events"], client, fn answer, client ->
        {[event], rest} = Enum.split_with(client.events, &(&1["id"] == answer["id"]))
        client = %{client | events: rest}

        case answer["status"] do
          "conflict" ->
            %{client | conflicts: client.conflicts ++ [Map.put(event, "conflicts", answer["conflicts"])]}

          _ ->
            client
        end
      end)

    # A rejected create drops the device's row, and the server's row, if
    # any, takes its place
    client =
      Enum.reduce(response["events"], client, fn answer, client ->
        client =
          if answer["status"] == "rejected" and created?(sent, answer["id"]),
            do: drop_row(client, answer["table"], answer["row_id"]),
            else: client

        case answer do
          %{"server" => nil, "table" => table, "row_id" => id} -> drop_row(client, table, id)
          %{"server" => server} -> apply_server_row(client, server)
        end
      end)

    {client, response}
  end

  # A row from the server never replaces one with a change still queued
  defp apply_server_row(client, %{"table" => table, "row" => %{"id" => id} = row}) do
    if {table, id} in client.outbox or Enum.any?(client.events, &(&1["row_id"] == id)),
      do: client,
      else: put_row(client, table, row)
  end

  # Posts, tags and comments have an owner; links and posts_tags are owned through
  # their post
  defp put_owner(row, table, user_id) when table in ["app_posts", "app_tags", "app_comments"],
    do: Map.put(row, "owner_id", user_id)

  defp put_owner(row, _table, _user_id), do: row

  defp local_row(client, table, id, now) do
    row(client, table, id) ||
      %{
        "id" => id,
        "version" => 0,
        "inserted_at" => now,
        "deleted_at" => nil,
        "deleted_by_id" => nil,
        "last_synced_at" => nil
      }
  end

  defp created?(events, id), do: Enum.any?(events, &(&1["id"] == id and &1["type"] == "create"))

  defp new_event(table, row_id, type, base_version, base, changes) do
    %{
      "id" => Ecto.UUID.generate(version: 7),
      "table" => table,
      "row_id" => row_id,
      "type" => type,
      "base_version" => base_version,
      "base" => base,
      "changes" => changes
    }
  end

  defp queue_event(client, event), do: %{client | events: client.events ++ [event]}

  defp put_row(client, table, %{"id" => id} = row) do
    %{client | rows: Map.update(client.rows, table, %{id => row}, &Map.put(&1, id, row))}
  end

  defp drop_row(client, table, id) do
    %{client | rows: Map.update(client.rows, table, %{}, &Map.delete(&1, id))}
  end

  # The device clock: now plus the offset, as the wire format, unless a
  # time is given.
  defp device_time(_client, %DateTime{} = at), do: DateTime.to_iso8601(at)

  defp device_time(%{clock_offset: offset}, nil) do
    DateTime.utc_now() |> DateTime.add(offset, :second) |> DateTime.to_iso8601()
  end

  # As JSON would carry it: string keys, datetimes as text.
  defp wire(term), do: term |> Jason.encode!() |> Jason.decode!()
end
