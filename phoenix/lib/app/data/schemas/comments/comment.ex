defmodule App.Data.Comment do
  @moduledoc """
  A comment on a post in a community: it belongs to the post's
  `posts_communities` row, so each community a post is in has its own
  thread. Written by a member of the community, who edits it; deleted by
  them or by the post's owner. See `App.Data.Commenting` and
  `docs/design/comments.md`.

  `changeset/3` is marked `@universal_changeset true`, see `App.Data.Post`.
  """

  use Ecto.Schema
  use App.Data.Synced
  use App.Data.UniversalChangeset
  import Ecto.Changeset

  @primary_key {:id, Ecto.UUID, autogenerate: [version: 7]}
  @foreign_key_type Ecto.UUID

  schema "app_comments" do
    belongs_to :post_community, App.Data.PostCommunity, foreign_key: :posts_communities_id
    belongs_to :owner, App.Data.AppUser
    field :body, :string
    field :last_synced_at, :utc_datetime_usec
    field :version, :integer, default: 1
    field :deleted_at, :utc_datetime_usec
    field :deleted_by_id, Ecto.UUID

    timestamps(type: :utc_datetime_usec)
  end

  @doc """
  Writing or editing a comment. The commenter is the `:owner_id` option,
  set by the flow, never a param.
  """
  @universal_changeset true
  def changeset(comment, params, options \\ []) do
    comment
    |> cast(params, [:posts_communities_id, :body])
    |> put_owner(Keyword.get(options, :owner_id))
    |> validate_required([:posts_communities_id, :owner_id, :body])
    |> validate_length(:body, max: 2000)
    |> put_change(:last_synced_at, DateTime.utc_now())
  end

  defp put_owner(changeset, nil), do: changeset
  defp put_owner(changeset, owner_id), do: put_change(changeset, :owner_id, owner_id)

  @doc """
  The fields an `update` event may change, see `App.Data.EventFlow`.
  """
  def event_changeset(comment, params, options \\ []) do
    comment
    |> changeset(params, options)
    |> cast(params, [:deleted_at])
    |> App.Data.Synced.put_deleted_by(Keyword.get(options, :deleted_by_id))
  end

  @doc """
  The rows above this one, see `App.Data.Deletes`: only the post in the
  community. The commenter's membership is not one, so a comment stays
  when its commenter leaves.
  """
  def parents, do: [posts_communities_id: App.Data.PostCommunity]

  @doc """
  The stream of the comment's events: its own, see `App.Data.EventLog`.
  """
  def event_stream(comment), do: {"app_comments", comment.id}
end
