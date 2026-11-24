defmodule App.Data.Link do
  @moduledoc """
  A link on a post: a label and a URL. A post has many, owned through it,
  so only the post's owner writes them, see `App.Data.Posting` and
  `docs/design/related-resources.md`.

  `changeset/3` is marked `@universal_changeset true`, see `App.Data.Post`.
  """

  use Ecto.Schema
  use App.Data.Synced
  use App.Data.UniversalChangeset
  import Ecto.Changeset

  @primary_key {:id, Ecto.UUID, autogenerate: [version: 7]}
  @foreign_key_type Ecto.UUID

  schema "app_links" do
    belongs_to :post, App.Data.Post
    field :label, :string
    field :url, :string
    field :last_synced_at, :utc_datetime_usec
    field :version, :integer, default: 1
    field :deleted_at, :utc_datetime_usec
    field :deleted_by_id, Ecto.UUID

    timestamps(type: :utc_datetime_usec)
  end

  @doc """
  Adding or editing a link.
  """
  @universal_changeset true
  def changeset(link, params, _options \\ []) do
    link
    |> cast(params, [:post_id, :label, :url])
    |> validate_required([:post_id, :label, :url])
    |> validate_length(:label, max: 100)
    |> validate_length(:url, max: 2000)
    |> validate_format(:url, ~r{\Ahttps?://[^\s/?#]+\S*\z}, message: "must be an http:// or https:// URL")
    |> put_change(:last_synced_at, DateTime.utc_now())
  end

  @doc """
  The fields an `update` event may change, see `App.Data.EventFlow`.
  """
  def event_changeset(link, params, options \\ []) do
    link
    |> changeset(params, options)
    |> cast(params, [:deleted_at])
    |> App.Data.Synced.put_deleted_by(Keyword.get(options, :deleted_by_id))
  end

  @doc """
  The rows above this one, see `App.Data.Deletes`.
  """
  def parents, do: [post_id: App.Data.Post]

  @doc """
  The stream of the link's events: its post's, see `App.Data.EventLog`.
  """
  def event_stream(link), do: {"app_posts", link.post_id}
end
