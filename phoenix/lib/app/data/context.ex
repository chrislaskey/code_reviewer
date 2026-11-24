defmodule App.Data.Context do
  @moduledoc """
  Common implementation for context functions. All database operations must go
  through the core Data contexts, so only universal logic that must always
  be run and always be validated should go through this. For use-case specific
  calls, make them in a separate module that calls this for the data layer
  actions.

  ## Examples

  Custom filters and sorts can be added by overriding the values:

    def filter(query, :hello, value), do: Ecto.Query.where(query, world: ^value)
    def filter(query, key, value), do: super(query, key, value)

  NOTE: the built-in `filter/3` function provides a dynamic fallback, matching
  key/value to schema fields. Make sure to include this clause if you want to
  continue using it.

  Or if you want to disable that functionality entirely, define this in the context:

    def filter(query, _key, _value), do: query

  ### Joins

  The `joins` key can be used to define named joins. The default implementation allows
  joining by any association defined in the schema. Joins can be referenced in
  other parts of the code, for example in a filter:

    use App.Data.Context, joins: [:users], #...

    def filter(query, :user_id, value) do
      Ecto.Query.where(query, [users: users], users.id == ^value)
    end

  Custom `join` functions can also be defined for more complex associations not
  defined in the schema:

    use App.Data.Context, joins: [:world], # ...

    def join(query, :world), do:
      query
      |> Ecto.Query.join(query, :left, [root], new in assoc(root, :hello), as: :hello)
      |> Ecto.Query.join(query, :left, [hello: hello], new in assoc(hello, :world), as: :world)
    end

    def filter(query, :world_id, value) do
      Ecto.Query.where(query, [world: world], world.id == ^value)
    end

    def filter(query, :world_slug, value) do
      Ecto.Query.where(query, [world: world], world.slug == ^value)
    end

  This defines a join once that every filter can then use, rather than (re)defining joins
  in the filter each time.

  ### Soft deletes

  A schema with a `deleted_at` field is soft-deleted: every read and
  `delete_many/1` here leaves out rows where it is set, so a caller cannot
  forget. A flow that revives a row, such as rejoining a community, asks
  for it with the `include_deleted: true` option.

  On such a schema `delete/2`, `delete_by/2` and `delete_many/1` soft-delete:
  they set `deleted_at`, now or the `deleted_at:` option, and the
  `deleted_by_id:` option. `delete/2` and `delete_by/2` go through the
  `:changesets` option when given, else the schema's `delete_changeset/3`
  when it has one. Without either, and in `delete_many/1`, a synced
  schema's `last_synced_at` is set too, so the tombstone reaches clients:

    Posts.delete(post, deleted_by_id: user.id)
    UsersCommunities.delete(membership, deleted_by_id: user.id, changesets: &UserCommunity.leave_changeset/3)

  The `hard: true` option deletes the row for good, soft-deleted or not.
  It is for admin tools and the purge of rows past the retention window:

    Posts.delete(post, hard: true)
    Posts.delete_many(filters: [deleted_at_before: horizon, last_synced_at_before: horizon], hard: true)

  A schema without `deleted_at` is always hard-deleted. See
  `docs/design/multi-tenancy.md`.

  """

  require Ecto.Query

  # Overridable

  @callback list(Keyword.t()) :: list(any())
  @callback join(map(), atom()) :: map()
  @callback filter(map(), atom(), any()) :: map()
  @callback sort(map(), atom(), any()) :: map()

  @callback get(String.t() | integer() | atom(), Keyword.t()) :: map() | nil
  @callback get_by(Keyword.t()) :: map() | nil
  @callback fetch(String.t() | integer() | atom(), Keyword.t()) ::
              {:ok, map()} | {:error, :not_found}
  @callback fetch_by(Keyword.t()) :: {:ok, map()} | {:error, :not_found}

  @callback change(struct(), map(), Keyword.t()) :: Ecto.Changeset.t()
  @callback create(map(), Keyword.t()) :: {:ok, map()} | {:error, any()}
  @callback update(struct(), map(), Keyword.t()) :: {:ok, map()} | {:error, any()}
  @callback get_or_create(String.t() | integer() | atom(), map(), Keyword.t()) ::
              {:ok, map()} | {:error, any()}
  @callback get_or_create_by(Keyword.t(), map(), Keyword.t()) :: {:ok, map()} | {:error, any()}
  @callback create_or_update(String.t() | integer() | atom(), map(), Keyword.t()) ::
              {:ok, map()} | {:error, any()}
  @callback create_or_update_by(Keyword.t(), map(), Keyword.t()) :: {:ok, map()} | {:error, any()}
  @callback delete(struct() | String.t() | integer(), Keyword.t()) :: {:ok, map()} | {:error, any()}
  @callback delete_by(Keyword.t(), Keyword.t()) :: {:ok, map()} | {:error, any()}
  @callback delete_many(Keyword.t()) :: {non_neg_integer(), nil | list(any())}

  # Not overridable

  @callback schema() :: atom()
  @callback joins() :: list(any())
  @callback preloads() :: list(any())
  @callback filters() :: list(any())
  @callback sorts() :: list(any())

  defmacro __using__(opts \\ []) do
    quote do
      @behaviour App.Data.Context

      require Ecto.Query

      # List

      def list(options \\ []) do
        schema()
        |> App.Data.Context.live(schema(), options)
        |> App.Data.Context.joins(joins(), options, &join/2)
        |> App.Data.Context.preloads(preloads(), options)
        |> App.Data.Context.filters(filters(), options, &filter/3)
        |> App.Data.Context.sorts(sorts(), options, &sort/3)
        |> App.Data.Context.limit(options)
        |> App.Repo.all()
      end

      def join(query, name) do
        App.Data.Context.join_by_any_schema_association(query, name)
      end

      def filter(query, key, value) do
        App.Data.Context.filter_by_any_schema_fields(query, key, value)
      end

      def sort(query, direction, field) do
        App.Data.Context.sort_by_any_schema_fields(query, direction, field)
      end

      # Get/fetch

      def get(id, options \\ []) do
        schema()
        |> App.Data.Context.live(schema(), options)
        |> App.Data.Context.joins(joins(), options, &join/2)
        |> App.Data.Context.preloads(preloads(), options)
        |> App.Data.Context.filters(filters(), options, &filter/3)
        |> App.Repo.get(id)
      end

      def get_by(values, options \\ []) do
        schema()
        |> App.Data.Context.live(schema(), options)
        |> App.Data.Context.joins(joins(), options, &join/2)
        |> App.Data.Context.preloads(preloads(), options)
        |> App.Data.Context.filters(filters(), options, &filter/3)
        |> App.Repo.get_by(values)
      end

      def fetch(id, options \\ []) do
        case get(id, options) do
          nil -> {:error, :not_found}
          value -> {:ok, value}
        end
      end

      def fetch_by(values, options \\ []) do
        case get_by(values, options) do
          nil -> {:error, :not_found}
          value -> {:ok, value}
        end
      end

      # Change

      def change(record, params \\ %{}, options \\ []) do
        App.Data.Context.changeset(schema(), record, params, options)
      end

      # Create

      def create(params, options \\ []) do
        schema()
        |> struct()
        |> change(params, options)
        |> App.Repo.insert()
      end

      # Update

      def update(record, params, options \\ []) do
        record
        |> change(params, options)
        |> App.Repo.update(stale_error_field: :version)
      end

      # Get or create

      def get_or_create(id, params, options \\ [])
      def get_or_create(%{id: id}, params, options), do: get_or_create(id, params, options)
      def get_or_create(%{"id" => id}, params, options), do: get_or_create(id, params, options)

      def get_or_create(id, params, options) do
        case get(id, options) do
          nil -> create(params, options)
          record -> record
        end
      end

      def get_or_create_by(by, params, options \\ []) do
        case get_by(by, options) do
          nil -> create(params, options)
          record -> record
        end
      end

      # Create or update

      def create_or_update(id, params, options \\ [])
      def create_or_update(%{id: id}, params, options), do: create_or_update(id, params, options)

      def create_or_update(%{"id" => id}, params, options),
        do: create_or_update(id, params, options)

      def create_or_update(id, params, options) do
        case get(id, options) do
          nil -> create(params, options)
          record -> update(record, params, options)
        end
      end

      def create_or_update_by(by, params, options \\ []) do
        case get_by(by, options) do
          nil -> create(params, options)
          record -> update(record, params, options)
        end
      end

      # Delete

      def delete(record, options \\ [])

      def delete(record, options) when is_map(record) do
        if App.Data.Context.soft_delete?(schema(), options) do
          record
          |> App.Data.Context.soft_delete_changeset(schema(), options)
          |> App.Repo.update(stale_error_field: :version)
        else
          App.Repo.delete(record)
        end
      end

      def delete(id, options) do
        case fetch(id, options) do
          {:ok, record} -> delete(record, options)
          error -> error
        end
      end

      def delete_by(by, options \\ []) do
        case fetch_by(by, options) do
          {:ok, record} -> delete(record, options)
          error -> error
        end
      end

      def delete_many(options) do
        if Keyword.get(options, :filters) == nil || Keyword.get(options, :filters) == [] do
          raise ArgumentError,
                "delete_many/1 needs at least one filter, it would delete every row"
        end

        query =
          schema()
          |> App.Data.Context.live(schema(), options)
          |> App.Data.Context.joins(joins(), options, &join/2)
          |> App.Data.Context.filters(filters(), options, &filter/3)

        if App.Data.Context.soft_delete?(schema(), options) do
          App.Repo.update_all(
            query,
            [set: App.Data.Context.soft_delete_fields(schema(), options)] ++
              App.Data.Context.version_inc(schema())
          )
        else
          App.Repo.delete_all(query)
        end
      end

      # Macro functions

      def schema, do: get_opt(:schema)
      def joins, do: get_opt(:joins, [])
      def preloads, do: get_opt(:preloads, [])
      def filters, do: get_opt(:filters, [])
      def sorts, do: get_opt(:sorts, [])

      # Macro helpers

      defp get_opt(key, default \\ nil) do
        Keyword.get(unquote(opts), key, default)
      end

      defoverridable list: 1,
                     join: 2,
                     filter: 3,
                     sort: 3,
                     get: 2,
                     get_by: 2,
                     fetch: 2,
                     fetch_by: 2,
                     change: 3,
                     create: 2,
                     update: 3,
                     get_or_create: 3,
                     get_or_create_by: 3,
                     create_or_update: 3,
                     create_or_update_by: 3,
                     delete: 2,
                     delete_by: 2,
                     delete_many: 1
    end
  end

  # Helpers - Soft deletes

  # A hard delete reaches soft-deleted rows too
  def live(query, schema, options \\ []) do
    include_deleted? = Keyword.get(options, :include_deleted, false) or Keyword.get(options, :hard, false)

    if :deleted_at in schema.__schema__(:fields) and not include_deleted? do
      Ecto.Query.where(query, [root], is_nil(root.deleted_at))
    else
      query
    end
  end

  def soft_delete?(schema, options) do
    :deleted_at in schema.__schema__(:fields) and not Keyword.get(options, :hard, false)
  end

  def soft_delete_changeset(record, schema, options) do
    fields = soft_delete_fields(schema, options)
    params = Map.new(Keyword.take(fields, [:deleted_at, :deleted_by_id]))

    cond do
      Keyword.has_key?(options, :changesets) ->
        changeset(schema, record, params, options)

      function_exported?(schema, :delete_changeset, 3) ->
        schema.delete_changeset(record, params, options) |> lock_version()

      true ->
        record |> Ecto.Changeset.change(fields) |> lock_version()
    end
  end

  # The columns a soft delete sets, with `last_synced_at` on a synced
  # schema so the tombstone reaches clients
  def soft_delete_fields(schema, options) do
    now = DateTime.utc_now()

    fields = [
      deleted_at: Keyword.get(options, :deleted_at, now),
      deleted_by_id: Keyword.get(options, :deleted_by_id)
    ]

    if :last_synced_at in schema.__schema__(:fields),
      do: fields ++ [last_synced_at: now],
      else: fields
  end

  # Helpers - Limit

  def limit(query, options) do
    case Keyword.get(options, :limit) do
      nil -> query
      count -> Ecto.Query.limit(query, ^count)
    end
  end

  # Helpers - Joins

  def joins(query, default, options, callback) do
    options
    |> Keyword.get(:joins, default)
    |> Enum.reduce(query, fn name, query ->
      callback.(query, name)
    end)
  end

  def join_by_any_schema_association(query, name) do
    if Ecto.Query.has_named_binding?(query, name) do
      query
    else
      Ecto.Query.join(query, :left, [root], new in assoc(root, ^name), as: ^name)
    end
  end

  # Helpers - Preloads

  def preloads(query, default, options) do
    preloads = Keyword.get(options, :preloads, default)

    Ecto.Query.preload(query, ^preloads)
  end

  # Helpers - Filters

  def filters(query, default, options, callback) do
    options
    |> Keyword.get(:filters, default)
    |> Enum.reduce(query, fn {key, value}, query ->
      callback.(query, key, value)
    end)
  end

  # The sync cursor of `App.Data.Sync`: rows written by the server after
  # `(last_synced_at, id)`, or after a `last_synced_at` alone. Available
  # to every context whose schema has the column.
  def filter_by_any_schema_fields(query, :synced_after, {%DateTime{} = at, nil}) do
    Ecto.Query.where(query, [root], root.last_synced_at > ^at)
  end

  def filter_by_any_schema_fields(query, :synced_after, {%DateTime{} = at, id}) do
    Ecto.Query.where(
      query,
      [root],
      root.last_synced_at > ^at or (root.last_synced_at == ^at and root.id > ^id)
    )
  end

  # The purge: rows the server last wrote before a time, and rows
  # soft-deleted before a time. Pair them with `hard: true`, which also
  # reaches soft-deleted rows.
  def filter_by_any_schema_fields(query, :last_synced_at_before, %DateTime{} = at) do
    Ecto.Query.where(query, [root], root.last_synced_at < ^at)
  end

  def filter_by_any_schema_fields(query, :deleted_at_before, %DateTime{} = at) do
    Ecto.Query.where(query, [root], not is_nil(root.deleted_at) and root.deleted_at < ^at)
  end

  def filter_by_any_schema_fields(query, key, values) when is_list(values) do
    Ecto.Query.where(query, [root], field(root, ^key) in ^values)
  end

  def filter_by_any_schema_fields(query, key, value) do
    Ecto.Query.where(query, ^Keyword.new([{key, value}]))
  end

  # Helpers - Sorts

  def sorts(query, default, options, callback) do
    options
    |> Keyword.get(:order_by, default)
    |> Enum.reduce(query, fn
      {direction, field}, query -> callback.(query, direction, field)
      field, query -> callback.(query, :asc, field)
    end)
  end

  def sort_by_any_schema_fields(query, direction, field) do
    Ecto.Query.order_by(query, ^Keyword.new([{direction, field}]))
  end

  # Helpers - Versions

  @doc """
  Increments `version` on an update of a stored row, which then fails
  with a `:version` error if the row changed since it was read.
  """
  def lock_version(%Ecto.Changeset{data: %schema{} = data} = changeset) do
    if :version in schema.__schema__(:fields) and Ecto.get_meta(data, :state) == :loaded,
      do: Ecto.Changeset.optimistic_lock(changeset, :version),
      else: changeset
  end

  @doc """
  The `update_all/3` option that increments `version`, when the schema has it.
  """
  def version_inc(schema) do
    if :version in schema.__schema__(:fields), do: [inc: [version: 1]], else: []
  end

  # Helpers - Changesets

  def changeset(schema, record, params, options) do
    changeset =
      case Keyword.get(options, :changesets) do
        nil ->
          schema.changeset(record, params, options)

        changesets ->
          changesets
          |> List.wrap()
          |> Enum.reduce(record, fn changeset, record -> changeset.(record, params, options) end)
      end

    lock_version(changeset)
  end
end
