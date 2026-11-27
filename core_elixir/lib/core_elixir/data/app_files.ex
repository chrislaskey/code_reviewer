defmodule CoreElixir.Data.AppFiles do
  @moduledoc """
  File records: the `app_files` schema, its baseline validation and its
  reads and writes. Only the files projector writes them, see
  `CoreElixir.App.Files.Projector`.
  """
  use Ecto.Schema
  import CoreElixir.Data.SyncedRow, only: [sync_columns: 0]
  import Ecto.Query
  alias CoreElixir.Data.SyncedRow
  alias CoreElixir.Helpers.Sha256
  alias CoreElixir.Repo

  @table "app_files"
  @max_bytes 25 * 1024 * 1024
  @max_content_bytes 64 * 1024
  @max_name_length 255
  @max_content_type_length 255

  @primary_key {:id, Ecto.UUID, autogenerate: false}
  schema @table do
    field :created_by_id, Ecto.UUID
    field :name, :string
    field :content_type, :string
    field :size, :integer
    field :sha256, :string
    field :content, :string
    sync_columns()
  end

  def table, do: @table

  @doc "The row as the wire carries it."
  def to_wire(file) do
    Map.merge(SyncedRow.wire(file), %{
      "created_by_id" => file.created_by_id,
      "name" => file.name,
      "content_type" => file.content_type,
      "size" => file.size,
      "sha256" => file.sha256,
      "content" => file.content
    })
  end

  @doc "Where the file's bytes are in the bucket."
  def storage_key(file), do: "files/#{file.created_by_id}/#{file.id}"

  @doc "The errors of a file's fields, by field, as the Rust validators give them; empty when it is valid."
  def field_errors(fields) do
    [
      {"name", name_error(fields["name"])},
      {"content_type", content_type_error(fields["content_type"])},
      {"size", size_error(fields["size"])},
      {"sha256", if(Sha256.valid?(fields["sha256"]), do: nil, else: "is not a SHA-256")},
      {"content", content_error(fields["content"])}
    ]
    |> Enum.reject(fn {_field, error} -> is_nil(error) end)
    |> Map.new(fn {field, error} -> {field, [error]} end)
  end

  @doc "The error of a name, or nil."
  def name_error(name) when is_binary(name) do
    cond do
      String.trim(name) == "" -> "is blank"
      String.length(name) > @max_name_length -> "is longer than 255 characters"
      true -> nil
    end
  end

  def name_error(_), do: "is blank"

  defp content_type_error(value) when is_binary(value) do
    essence = value |> String.split(";") |> hd() |> String.trim()

    well_formed =
      case String.split(essence, "/") do
        [kind, subtype] -> kind != "" and subtype != ""
        _ -> false
      end

    printable = String.match?(value, ~r/\A[\x20-\x7e]*\z/)

    if well_formed and printable and byte_size(value) <= @max_content_type_length,
      do: nil,
      else: "is not a content type"
  end

  defp content_type_error(_), do: "is not a content type"

  defp size_error(size) when is_integer(size) and size > @max_bytes, do: "is larger than 25 MB"
  defp size_error(size) when is_integer(size) and size > 0, do: nil
  defp size_error(_), do: "is empty"

  defp content_error(nil), do: nil
  defp content_error(""), do: "is empty"

  defp content_error(content) when is_binary(content) and byte_size(content) > @max_content_bytes,
    do: "is larger than 64 KB"

  defp content_error(content) when is_binary(content), do: nil
  defp content_error(_), do: "is empty"

  @doc "Gets a row by id, deleted or not."
  def get(id), do: Repo.get(__MODULE__, id)

  @doc "Gets a live file the user sees, or nil."
  def get_visible(user_id, id) do
    Repo.one(from f in __MODULE__, where: f.id == ^id and is_nil(f.deleted_at), where: ^visible_to(user_id))
  end

  @doc """
  Counts the live files the user sees whose names contain `name` in any
  case, of one of `content_types` (any for none), and lists `limit` of
  them from `offset`, newest first.
  """
  def search(user_id, name, content_types, offset, limit) do
    query =
      from f in __MODULE__,
        where: is_nil(f.deleted_at),
        where: ^visible_to(user_id),
        where: fragment("strpos(lower(?), lower(?)) > 0", f.name, ^name)

    query =
      case content_types do
        [] -> query
        types -> where(query, [f], fragment("trim(split_part(?, ';', 1))", f.content_type) in ^types)
      end

    total = Repo.aggregate(query, :count)
    files = Repo.all(from f in query, order_by: [desc: f.inserted_at, desc: f.id], offset: ^offset, limit: ^limit)
    {total, files}
  end

  @doc "Lists the live files the user sees with any of the checksums, newest first."
  def list_with_sha256s(user_id, sha256s) do
    Repo.all(
      from f in __MODULE__,
        where: is_nil(f.deleted_at) and f.sha256 in ^sha256s,
        where: ^visible_to(user_id),
        order_by: [desc: f.inserted_at, desc: f.id]
    )
  end

  @doc "Lists every row's creator and id, deleted or not, for the bucket sweep."
  def list_storage_keys do
    Repo.all(from f in __MODULE__, select: %{id: f.id, created_by_id: f.created_by_id})
    |> Enum.map(&storage_key/1)
  end

  @doc "Inserts a new row, or writes over its stored one, and publishes it."
  def create(file), do: SyncedRow.write(file, @table)
  def update(file), do: SyncedRow.write(file, @table)

  @doc "The rows the user sees with a version in the range."
  def list_synced_after(user_id, after_version, through, limit) do
    Repo.all(
      from f in __MODULE__,
        where: f.global_sync_version > ^after_version and f.global_sync_version <= ^through,
        where: ^visible_to(user_id),
        order_by: f.global_sync_version,
        limit: ^limit
    )
  end

  # A file with a live row in a community the user is a live member of
  defp visible_to(user_id) do
    dynamic(
      [f],
      fragment(
        "? IN (SELECT fc.file_id FROM app_files_communities fc WHERE fc.deleted_at IS NULL AND fc.community_id IN (SELECT community_id FROM app_users_communities WHERE user_id = ? AND deleted_at IS NULL))",
        f.id,
        type(^user_id, Ecto.UUID)
      )
    )
  end
end
