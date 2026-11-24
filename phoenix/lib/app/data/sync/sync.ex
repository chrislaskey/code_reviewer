defmodule App.Data.Sync do
  @moduledoc """
  The sync server of `docs/design/offline-sync.md`: pull, push and
  backfill for the scope's user, over the data contexts. `App.Web.SyncController` serves it over HTTP.

  Rows travel as `%{"table" => name, "row" => columns}`, the columns being
  the table's, with datetimes as UTC ISO 8601 text with six fractional
  digits. The synced tables are `tables/0`, parents first.

  ## Pull

  The rows visible to the user with `last_synced_at` later than the
  cursor, every table merged and ordered by `(last_synced_at, id)`,
  paged. A `posts_communities` row carries its post and the post's owner,
  regardless of their `last_synced_at`. The request's `cursor` is what the
  client stored after its last sync, pulled from an overlap buffer
  earlier so a write that committed late is not missed; `after` is the
  exact page token of the previous response, for the following pages. The
  response has `rows`, `next`, the token of the next page or nil, and
  `cursor`, what to store once `next` is nil.

  ## Push

  One transaction per batch of `rows` and `events`, parents first. Every
  datetime is checked for the exact format, client datetimes are bounded
  by the server clock, and everything is authorized against the scope.
  A user-editable table, `@event_tables`, changes only through events,
  merged field by field by its flow's `apply_event/2`. Other rows are
  server-wins: app users, memberships and `posts_communities` rows, and a
  community once a client has created it. The response lists `accepted`
  and `rejected` rows, and an answer per event. See
  `docs/design/offline-sync.md`.

  ## Backfill

  Every live `posts_communities` row of a community the user is a member
  of, each carrying its post, paged like a pull. A client calls it once
  when it sees a new membership of its own user.
  """

  alias App.Data.AppUser
  alias App.Data.AppUsers
  alias App.Data.Comment
  alias App.Data.Commenting
  alias App.Data.Comments
  alias App.Data.Communities
  alias App.Data.Community
  alias App.Data.Link
  alias App.Data.Links
  alias App.Data.Memberships
  alias App.Data.Post
  alias App.Data.PostCommunity
  alias App.Data.Posting
  alias App.Data.Posts
  alias App.Data.PostsCommunities
  alias App.Data.PostsTags
  alias App.Data.PostTag
  alias App.Data.Profiles
  alias App.Data.Sync.Merge
  alias App.Data.Tag
  alias App.Data.Tagging
  alias App.Data.Tags
  alias App.Data.UserCommunity
  alias App.Data.UserProfile
  alias App.Data.UserProfiles
  alias App.Data.UsersCommunities
  alias App.Web.Scope

  @overlap_seconds 60
  @default_limit 100
  @max_limit 500
  @epoch ~U[1970-01-01 00:00:00.000000Z]
  @datetime ~r/^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}\.\d{6}Z$/

  # Parents first, which is the push order.
  @tables [
    {"app_users", AppUser, AppUsers},
    {"app_user_profiles", UserProfile, UserProfiles},
    {"app_communities", Community, Communities},
    {"app_users_communities", UserCommunity, UsersCommunities},
    {"app_posts", Post, Posts},
    {"app_posts_communities", PostCommunity, PostsCommunities},
    {"app_links", Link, Links},
    {"app_tags", Tag, Tags},
    {"app_posts_tags", PostTag, PostsTags},
    {"app_comments", Comment, Comments}
  ]

  # The tables a client changes only through events, with the flow that
  # applies them
  @event_tables %{
    "app_user_profiles" => {UserProfile, Profiles},
    "app_posts" => {Post, Posting},
    "app_links" => {Link, Posting},
    "app_tags" => {Tag, Tagging},
    "app_posts_tags" => {PostTag, Tagging},
    "app_comments" => {Comment, Commenting}
  }

  @doc """
  The synced tables as `{name, schema, context}`, parents first.
  """
  def tables, do: @tables

  @doc """
  The synced schemas, parents first.
  """
  def schemas, do: Enum.map(@tables, &elem(&1, 1))

  @doc """
  How far before the stored cursor a pull starts, in seconds.
  """
  def overlap_seconds, do: @overlap_seconds

  ## Pull

  def pull(%Scope{} = scope, params) do
    horizon = App.Data.Purge.horizon()

    if resync?(params, horizon) do
      {:ok, %{"rows" => [], "next" => nil, "cursor" => nil, "purge_horizon" => horizon, "resync" => true}}
    else
      page = scope |> pull_page(params) |> Map.merge(%{"purge_horizon" => horizon, "resync" => false})
      {:ok, page}
    end
  rescue
    ArgumentError -> {:error, :bad_cursor}
  end

  # A client whose cursor is older than the purge horizon may have missed
  # a tombstone, so it drops what it has and pulls from nothing.
  defp resync?(%{"cursor" => %{"last_synced_at" => at}}, %DateTime{} = horizon) when is_binary(at) do
    DateTime.compare(parse_datetime!(at), horizon) == :lt
  end

  defp resync?(_params, _horizon), do: false

  defp pull_page(scope, params) do
    limit = limit(params)
    cursor = request_cursor(params)

    rows =
      for {table, _schema, context} <- @tables,
          row <-
            context.list(
              filters: [visible_to: scope, synced_after: cursor],
              include_deleted: true,
              order_by: [:last_synced_at, :id],
              limit: limit + 1
            ),
          do: {table, row}

    page(rows, limit, params["cursor"])
  end

  ## Backfill

  def backfill(%Scope{} = scope, community_id, params) do
    if Memberships.member?(scope, community_id) do
      limit = limit(params)

      rows =
        for row <-
              PostsCommunities.list(
                filters: [community_id: community_id, synced_after: page_cursor(params)],
                order_by: [:last_synced_at, :id],
                limit: limit + 1
              ),
            do: {"app_posts_communities", row}

      {:ok, page(rows, limit, nil)}
    else
      {:error, :not_a_member}
    end
  rescue
    ArgumentError -> {:error, :bad_cursor}
  end

  # One page of `{table, row}` pairs: the first `limit` in cursor order,
  # carried rows included, with the token of the next page.
  defp page(rows, limit, stored_cursor) do
    sorted = Enum.sort_by(rows, fn {_table, row} -> sort_key(row) end)
    {page, rest} = Enum.split(sorted, limit)

    cursor =
      case List.last(page) do
        nil -> stored_cursor
        {_table, row} -> %{"last_synced_at" => row.last_synced_at, "id" => row.id}
      end

    %{
      "rows" => page |> carry() |> Enum.map(&encode/1),
      "next" => if(rest != [], do: cursor),
      "cursor" => cursor
    }
  end

  defp sort_key(row), do: {DateTime.to_unix(row.last_synced_at, :microsecond), row.id}

  # Every live `posts_communities` row carries its post and the post's
  # owner, placed before it so a client with foreign keys can apply in
  # order, and after it the post's live links, tags and posts_tags rows,
  # tags first, then the row's live comments, their authors first. A
  # removed row carries nothing: a client that held the
  # post prunes it, and one that never did must not receive it now.
  defp carry(page) do
    post_ids = for {"app_posts_communities", %{deleted_at: nil} = row} <- page, uniq: true, do: row.post_id

    posts =
      [filters: [id: post_ids], include_deleted: true]
      |> Posts.list()
      |> Map.new(&{&1.id, &1})

    owner_ids = posts |> Map.values() |> Enum.map(& &1.owner_id) |> Enum.uniq()

    owners =
      [filters: [id: owner_ids], include_deleted: true]
      |> AppUsers.list()
      |> Map.new(&{&1.id, &1})

    links = [filters: [post_id: post_ids], order_by: [:id]] |> Links.list() |> Enum.group_by(& &1.post_id)

    posts_tags =
      [filters: [post_id: post_ids], order_by: [:id]] |> PostsTags.list() |> Enum.group_by(& &1.post_id)

    tags =
      [filters: [id: posts_tags |> Map.values() |> List.flatten() |> Enum.map(& &1.tag_id)]]
      |> Tags.list()
      |> Map.new(&{&1.id, &1})

    thread_ids = for {"app_posts_communities", %{deleted_at: nil} = row} <- page, do: row.id

    comments =
      [filters: [posts_communities_id: thread_ids], preloads: [:owner]]
      |> Comments.list()
      |> Enum.group_by(& &1.posts_communities_id)

    page
    |> Enum.flat_map(fn
      {"app_posts_communities", %{deleted_at: nil} = row} = pair ->
        post = Map.fetch!(posts, row.post_id)
        its_tags = Map.get(posts_tags, post.id, [])

        [{"app_users", Map.fetch!(owners, post.owner_id)}, {"app_posts", post}, pair] ++
          for(link <- Map.get(links, post.id, []), do: {"app_links", link}) ++
          for(row <- its_tags, do: {"app_tags", Map.fetch!(tags, row.tag_id)}) ++
          for(row <- its_tags, do: {"app_posts_tags", row}) ++
          carry_comments(Map.get(comments, row.id, []))

      pair ->
        [pair]
    end)
    |> Enum.uniq_by(fn {table, row} -> {table, row.id} end)
  end

  defp carry_comments(comments) do
    for(comment <- comments, do: {"app_users", comment.owner}) ++
      for(comment <- comments, do: {"app_comments", comment})
  end

  defp limit(params) do
    case params["limit"] do
      count when is_integer(count) and count > 0 -> min(count, @max_limit)
      _ -> @default_limit
    end
  end

  # The exact page token when there is one, else the stored cursor from
  # the overlap buffer earlier, else the beginning.
  defp request_cursor(%{"after" => %{} = token}), do: page_cursor(%{"after" => token})

  defp request_cursor(%{"cursor" => %{"last_synced_at" => at}}) when is_binary(at) do
    {DateTime.add(parse_datetime!(at), -@overlap_seconds, :second), nil}
  end

  defp request_cursor(_params), do: {@epoch, nil}

  defp page_cursor(%{"after" => %{"last_synced_at" => at, "id" => id}})
       when is_binary(at) and is_binary(id),
       do: {parse_datetime!(at), id}

  defp page_cursor(_params), do: {@epoch, nil}

  defp parse_datetime!(text) do
    case DateTime.from_iso8601(text) do
      {:ok, datetime, 0} -> datetime
      _ -> raise ArgumentError, "not a UTC datetime: #{inspect(text)}"
    end
  end

  ## Push

  def push(%Scope{} = scope, rows, events \\ []) when is_list(rows) and is_list(events) do
    results = run(scope, rows, events)

    %{
      "accepted" =>
        for(
          {:accepted, table, id, row} <- results,
          do: %{
            "table" => table,
            "id" => id,
            "last_synced_at" => row.last_synced_at,
            "version" => row.version
          }
        ),
      "rejected" =>
        for {:rejected, table, id, reason, server} <- results do
          %{"table" => table, "id" => id, "reason" => reason, "server" => server}
        end,
      "events" => for({:event, answer, _result} <- results, do: answer)
    }
  end

  @doc """
  What `push/3` does, in one transaction, before it is put on the wire:
  for each row and event, parents first, `{:accepted, table, id, row}`,
  `{:rejected, table, id, reason, server_row}`, or `{:event, answer,
  result}` with the flow's own result beside the answer. For the web,
  which pushes as a device does and wants the structs back, see `save/4`.
  """
  def run(%Scope{} = scope, rows, events) when is_list(rows) and is_list(events) do
    {:ok, results} =
      App.Repo.transact(fn ->
        {:ok,
         Enum.flat_map(batches(rows, events), fn
           {:row, row} -> [apply_row(scope, row)]
           {:event, event} -> [apply_event(scope, event)]
         end)}
      end)

    results
  end

  @doc """
  The web's save of one row: the event a device would send, from `loaded`,
  the row as the page loaded it, or nil for a new row, applied at once.
  `changes` are by field name, as strings. Returns `{:ok, row}`,
  `{:conflict, conflicts, row}` when someone changed the same field
  first, `{:error, changeset}`, or `{:error, reason}`.
  """
  def save(%Scope{} = scope, table, loaded, changes) do
    changes = Map.new(changes, fn {field, value} -> {to_string(field), Merge.wire(value)} end)

    event =
      case loaded do
        nil ->
          event(table, Ecto.UUID.generate(version: 7), "create", 0, %{}, changes)

        row ->
          base = Merge.values(row, Map.keys(changes))
          event(table, row.id, "update", row.version, base, changes)
      end

    [{:event, _answer, result}] = run(scope, [], [event])

    case result do
      {:ok, %{conflicts: conflicts}, row} when conflicts == %{} -> {:ok, row}
      {:ok, %{conflicts: conflicts}, row} -> {:conflict, conflicts, row}
      {:error, reason, _row} -> {:error, reason}
      {:error, reason} -> {:error, reason}
    end
  end

  @doc """
  An event as a device sends it.
  """
  def event(table, row_id, type, base_version, base, changes) do
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

  @doc """
  The results of `run/3` as one outcome, for a save the web makes all or
  nothing: `:ok` when everything applied, including a server-wins row
  that comes back as the server has it, else the first error, as
  `{:error, changeset}` or `{:error, reason}` with the reasons the web
  handles as atoms.
  """
  def outcome(results) do
    Enum.find_value(results, :ok, fn
      {:accepted, _table, _id, _row} -> nil
      {:rejected, _table, _id, "server_wins", _server} -> nil
      {:rejected, _table, _id, reason, _server} -> {:error, reason(reason)}
      {:event, _answer, {:ok, _recorded, _row}} -> nil
      {:event, _answer, {:error, %Ecto.Changeset{} = changeset, _row}} -> {:error, changeset}
      {:event, _answer, {:error, reason, _row}} -> {:error, reason(reason)}
      {:event, _answer, {:error, reason}} -> {:error, reason(reason)}
    end)
  end

  defp reason(reason) when reason in ["not_a_member", "not_owner"], do: String.to_existing_atom(reason)
  defp reason(reason), do: reason

  defp batches(rows, events) do
    order = @tables |> Enum.map(&elem(&1, 0)) |> Enum.with_index() |> Map.new()
    rank = fn table, kind -> {Map.get(order, table, map_size(order)), kind} end

    Enum.sort_by(
      Enum.map(events, &{:event, &1}) ++ Enum.map(rows, &{:row, &1}),
      fn
        {:event, event} -> rank.(table_of(event), 0)
        {:row, row} -> rank.(table_of(row), 1)
      end
    )
  end

  defp apply_row(scope, %{"table" => table, "row" => %{"id" => id} = row}) do
    with {:ok, schema, context} <- lookup(table),
         {:ok, incoming} <- decode(schema, row) do
      apply_row(scope, table, context, incoming)
    else
      {:error, reason} -> {:rejected, table, id, reason, nil}
    end
  end

  defp apply_row(_scope, other) do
    {:rejected, table_of(other), id_of(other), "malformed", nil}
  end

  defp table_of(%{"table" => table}) when is_binary(table), do: table
  defp table_of(_other), do: nil

  defp id_of(%{"row" => %{"id" => id}}) when is_binary(id), do: id
  defp id_of(_other), do: nil

  defp lookup(table) do
    case List.keyfind(@tables, table, 0) do
      {^table, schema, context} -> {:ok, schema, context}
      nil -> {:error, "unknown_table"}
    end
  end

  # The row as the schema's struct: every datetime in the exact format,
  # every column cast, the schema's own validations applied.
  defp decode(schema, row) do
    with :ok <- check_datetimes(schema, row) do
      changeset =
        schema
        |> struct()
        |> Ecto.Changeset.cast(row, schema.__schema__(:fields) -- [:last_synced_at])
        |> schema.changeset(row)
        |> Ecto.Changeset.validate_required([:id, :updated_at])

      if changeset.valid? do
        {:ok, Ecto.Changeset.apply_changes(changeset)}
      else
        {:error,
         "invalid: " <>
           Enum.map_join(changeset.errors, ", ", fn {field, {message, _}} -> "#{field} #{message}" end)}
      end
    end
  end

  defp check_datetimes(schema, row) do
    bad =
      for field <- schema.__schema__(:fields),
          schema.__schema__(:type, field) == :utc_datetime_usec,
          value = row[Atom.to_string(field)],
          value != nil,
          not (is_binary(value) and Regex.match?(@datetime, value)),
          do: field

    case bad do
      [] -> :ok
      fields -> {:error, "bad_datetime: " <> Enum.join(fields, ", ")}
    end
  end

  defp apply_row(_scope, table, _context, incoming) when is_map_key(@event_tables, table) do
    {:rejected, table, incoming.id, "events_only", nil}
  end

  # App users come from registration and are never accepted from a client.
  defp apply_row(_scope, "app_users", context, incoming) do
    {:rejected, "app_users", incoming.id, "server_wins", server_version(context, incoming.id)}
  end

  # A client creates a community, as any user may,
  # and never changes one: the web app has no edit or delete either, and
  # who may will be a role, later. Server-wins until then.
  defp apply_row(scope, "app_communities", context, incoming) do
    case context.get(incoming.id, include_deleted: true) do
      nil ->
        insert(scope, "app_communities", Community, incoming)

      existing ->
        {:rejected, "app_communities", incoming.id, "server_wins", encode({"app_communities", existing})}
    end
  end

  # Server-wins: the client's row is an intent, applied through the flow,
  # and the server's row comes back for the client to apply.
  defp apply_row(%Scope{user: user} = scope, "app_users_communities", context, incoming) do
    result =
      cond do
        incoming.user_id != user.id -> {:error, "not_own_membership"}
        incoming.deleted_at == nil -> Memberships.join(scope, incoming.community_id)
        true -> leave(scope, incoming.community_id)
      end

    server_wins("app_users_communities", context, incoming, result)
  end

  defp apply_row(scope, "app_posts_communities", context, incoming) do
    result =
      case Posts.get(incoming.post_id, include_deleted: true) do
        nil -> {:error, "unknown_post"}
        post when incoming.deleted_at == nil -> Posting.add_to_community(scope, post, incoming.community_id)
        post -> remove(scope, post, incoming.community_id)
      end

    server_wins("app_posts_communities", context, incoming, result)
  end

  defp leave(scope, community_id) do
    case Memberships.leave(scope, community_id) do
      {:error, :not_found} -> {:ok, nil}
      result -> result
    end
  end

  defp remove(scope, post, community_id) do
    case Posting.remove_from_community(scope, post, community_id) do
      {:error, :not_found} -> {:ok, nil}
      result -> result
    end
  end

  defp server_wins(table, context, incoming, result) do
    case result do
      {:ok, nil} ->
        {:rejected, table, incoming.id, "server_wins", server_version(context, incoming.id)}

      {:ok, row} ->
        {:rejected, table, incoming.id, "server_wins", encode({table, row})}

      {:error, reason} ->
        {:rejected, table, incoming.id, to_string(reason), server_version(context, incoming.id)}
    end
  end

  defp server_version(context, id) do
    case context.get(id, include_deleted: true) do
      nil -> nil
      row -> encode({context.schema().__schema__(:source), row})
    end
  end

  defp insert(%Scope{user: user}, table, schema, incoming) do
    now = DateTime.utc_now()

    values =
      incoming
      |> bound(now, user)
      |> Map.take(schema.__schema__(:fields))
      |> Map.merge(%{version: 1, last_synced_at: now})
      |> Map.update!(:inserted_at, &(&1 || now))

    App.Repo.insert_all(schema, [values])
    {:accepted, table, incoming.id, values}
  end

  # No datetime from a client is later than now, and a client deletes
  # only as itself.
  defp bound(incoming, now, user) do
    incoming
    |> Map.update!(:updated_at, &earliest(&1, now))
    |> Map.update!(:inserted_at, &earliest(&1, now))
    |> Map.update!(:deleted_at, &earliest(&1, now))
    |> Map.update!(:deleted_by_id, &if(incoming.deleted_at, do: user.id, else: &1 && nil))
  end

  defp earliest(nil, _now), do: nil
  defp earliest(datetime, now), do: if(DateTime.compare(datetime, now) == :gt, do: now, else: datetime)

  ## Events

  defp apply_event(scope, event) do
    answer = %{
      "id" => string_of(event, "id"),
      "table" => table_of(event),
      "row_id" => string_of(event, "row_id")
    }

    {answer_of_result, result} = event_result(scope, table_of(event), event)
    {:event, Map.merge(answer, answer_of_result), result}
  end

  defp event_result(scope, table, event) do
    with {:ok, schema, flow} <- event_flow(table),
         {:ok, checked} <- check_event(schema, table, event) do
      result = flow.apply_event(scope, checked)
      {event_answer(table, result), result}
    else
      {:error, reason} -> {event_rejected(reason, nil), {:error, reason}}
    end
  end

  defp event_answer(table, result) do
    case result do
      {:ok, recorded, row} ->
        %{
          "status" => if(recorded.conflicts == %{}, do: "applied", else: "conflict"),
          "version" => recorded.version,
          "conflicts" => recorded.conflicts,
          "server" => encode({table, row})
        }

      {:error, %Ecto.Changeset{} = changeset, row} ->
        Map.put(event_rejected(invalid(changeset), row && encode({table, row})), "errors", errors(changeset))

      {:error, %Ecto.Changeset{} = changeset} ->
        Map.put(event_rejected(invalid(changeset), nil), "errors", errors(changeset))

      {:error, reason, row} ->
        event_rejected(reason, row && encode({table, row}))
    end
  end

  defp invalid(changeset) do
    "invalid: " <>
      Enum.map_join(changeset.errors, ", ", fn {field, {message, _}} -> "#{field} #{message}" end)
  end

  # Each field's messages with their values filled in, for a form
  defp errors(changeset) do
    Ecto.Changeset.traverse_errors(changeset, fn {message, options} ->
      Regex.replace(~r"%{(\w+)}", message, fn _match, key ->
        options |> Keyword.get(String.to_existing_atom(key), key) |> to_string()
      end)
    end)
  end

  defp event_rejected(reason, server), do: %{"status" => "rejected", "reason" => reason, "server" => server}

  defp event_flow(table) do
    case Map.fetch(@event_tables, table) do
      {:ok, {schema, flow}} -> {:ok, schema, flow}
      :error -> {:error, "not_an_event_table"}
    end
  end

  # A deletion time is bounded like a row's.
  defp check_event(
         schema,
         table,
         %{"id" => id, "row_id" => row_id, "type" => type, "base_version" => base_version} = event
       )
       when type in ["create", "update"] and is_integer(base_version) and base_version >= 0 do
    base = Map.get(event, "base") || %{}
    changes = Map.get(event, "changes")

    with {:ok, id} <- Ecto.UUID.cast(id),
         {:ok, row_id} <- Ecto.UUID.cast(row_id),
         true <- is_map(base) and is_map(changes) and map_size(changes) > 0,
         :ok <- check_datetimes(schema, base),
         :ok <- check_datetimes(schema, changes) do
      {:ok,
       %{
         id: id,
         table: table,
         row_id: row_id,
         type: String.to_existing_atom(type),
         base_version: base_version,
         base: base,
         changes: bound_changes(changes)
       }}
    else
      {:error, reason} when is_binary(reason) -> {:error, reason}
      _ -> {:error, "malformed"}
    end
  end

  defp check_event(_schema, _table, _event), do: {:error, "malformed"}

  defp bound_changes(%{"deleted_at" => at} = changes) when is_binary(at) do
    now = DateTime.utc_now()
    Map.put(changes, "deleted_at", at |> parse_datetime!() |> earliest(now) |> DateTime.to_iso8601())
  end

  defp bound_changes(changes), do: changes

  defp string_of(map, key) do
    case map do
      %{^key => value} when is_binary(value) -> value
      _ -> nil
    end
  end

  ## Rows on the wire

  @doc """
  A `{table, struct}` pair as `%{"table" => name, "row" => columns}`.
  """
  def encode({"app_posts", %Post{deleted_at: %DateTime{}} = post}) do
    # A tombstone travels without its content
    encode_fields({"app_posts", %{post | body: ""}})
  end

  def encode(pair), do: encode_fields(pair)

  defp encode_fields({table, %schema{} = row}) do
    %{"table" => table, "row" => Map.take(row, schema.__schema__(:fields))}
  end
end
