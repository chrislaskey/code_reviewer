defmodule App.Data.PostTag do
  @moduledoc """
  "This post has this tag", with the tag's `position` among the post's
  tags and a `note`. The post's owner, who is also the tag's, writes it,
  see `App.Data.Tagging`. A pair has one row for good: removing the tag
  soft-deletes it, and tagging again revives it, so the unique index on
  the pair is plain. See `docs/design/related-resources.md`.

  `changeset/3` is marked `@universal_changeset true`, see `App.Data.Post`.
  """

  use Ecto.Schema
  use App.Data.Synced
  use App.Data.UniversalChangeset
  import Ecto.Changeset

  @primary_key {:id, Ecto.UUID, autogenerate: [version: 7]}
  @foreign_key_type Ecto.UUID

  schema "app_posts_tags" do
    belongs_to :post, App.Data.Post
    belongs_to :tag, App.Data.Tag
    field :position, :integer
    field :note, :string
    field :last_synced_at, :utc_datetime_usec
    field :version, :integer, default: 1
    field :deleted_at, :utc_datetime_usec
    field :deleted_by_id, Ecto.UUID

    timestamps(type: :utc_datetime_usec)
  end

  @doc """
  Tagging a post, or changing the tag's position or note.
  """
  @universal_changeset true
  def changeset(row, params, _options \\ []) do
    row
    |> cast(params, [:post_id, :tag_id, :position, :note])
    |> validate_required([:post_id, :tag_id, :position])
    |> validate_number(:position, greater_than_or_equal_to: 0)
    |> validate_length(:note, max: 200)
    |> unique_constraint([:post_id, :tag_id])
    |> put_change(:last_synced_at, DateTime.utc_now())
  end

  @doc """
  The fields an `update` event may change, see `App.Data.EventFlow`.
  """
  def event_changeset(row, params, options \\ []) do
    row
    |> changeset(params, options)
    |> cast(params, [:deleted_at])
    |> App.Data.Synced.put_deleted_by(Keyword.get(options, :deleted_by_id))
  end

  @doc """
  The rows above this one, see `App.Data.Deletes`.
  """
  def parents, do: [post_id: App.Data.Post, tag_id: App.Data.Tag]

  @doc """
  The stream of the row's events: its post's, see `App.Data.EventLog`.
  """
  def event_stream(row), do: {"app_posts", row.post_id}
end
