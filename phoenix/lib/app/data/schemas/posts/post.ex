defmodule App.Data.Post do
  @moduledoc """
  A post: the first resource, written by one owner and in the
  communities the owner chose, see `App.Data.PostCommunity`, with its
  links and tags, see `docs/design/related-resources.md`. The reference
  for every rule a resource follows, see phase 3 of
  `docs/design/build-order.md`.

  `last_synced_at` is set on every write, as the sync protocol in
  `docs/design/offline-sync.md` requires of every synced table.

  `changeset/3` is marked `@universal_changeset true`: `mix app.rust` writes
  it as `post.rs` for the desktop and mobile apps, and this app runs that
  Rust too, see `priv/rust/README.md`.
  """

  use Ecto.Schema
  use App.Data.Synced
  use App.Data.UniversalChangeset
  import Ecto.Changeset

  @primary_key {:id, Ecto.UUID, autogenerate: [version: 7]}
  @foreign_key_type Ecto.UUID

  schema "app_posts" do
    belongs_to :owner, App.Data.AppUser
    has_many :links, App.Data.Link
    has_many :posts_tags, App.Data.PostTag
    field :body, :string
    field :last_synced_at, :utc_datetime_usec
    field :version, :integer, default: 1
    field :deleted_at, :utc_datetime_usec
    field :deleted_by_id, Ecto.UUID

    timestamps(type: :utc_datetime_usec)
  end

  @doc """
  Writing or editing a post. The owner is the `:owner_id` option, set
  by the flow, never a param.
  """
  @universal_changeset true
  def changeset(post, params, options \\ []) do
    post
    |> cast(params, [:body])
    |> put_owner(Keyword.get(options, :owner_id))
    |> validate_required([:owner_id, :body])
    |> validate_length(:body, max: 10_000)
    |> put_change(:last_synced_at, DateTime.utc_now())
  end

  defp put_owner(changeset, nil), do: changeset
  defp put_owner(changeset, owner_id), do: put_change(changeset, :owner_id, owner_id)

  @doc """
  The fields an `update` event may change, see `App.Data.Posting.apply_event/2`.
  """
  def event_changeset(post, params, options \\ []) do
    post
    |> changeset(params, options)
    |> cast(params, [:deleted_at])
    |> App.Data.Synced.put_deleted_by(Keyword.get(options, :deleted_by_id))
  end

  @doc """
  The stream of the post's events: its own, see `App.Data.EventLog`.
  """
  def event_stream(post), do: {"app_posts", post.id}

  @doc """
  Deleting a post: a soft delete. `App.Data.Deletes` copies it onto the
  rows below.
  """
  def delete_changeset(post, params, _options \\ []) do
    post
    |> cast(params, [:deleted_at, :deleted_by_id])
    |> validate_required([:deleted_at, :deleted_by_id])
    |> put_change(:last_synced_at, DateTime.utc_now())
  end
end
