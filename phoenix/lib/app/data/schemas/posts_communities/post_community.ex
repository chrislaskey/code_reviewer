defmodule App.Data.PostCommunity do
  @moduledoc """
  "This post is in this community", and the membership it was added
  under, `users_communities_id`, which is always the owner's. Deleting
  the post or leaving the community copies its `deleted_at` here, and
  undoing either revives the row, see `App.Data.Deletes`. Removing a post from
  a community soft-deletes its row, and adding it back revives the same
  row, so the unique index on the pair is plain.
  """

  use Ecto.Schema
  use App.Data.Synced
  import Ecto.Changeset

  @primary_key {:id, Ecto.UUID, autogenerate: [version: 7]}
  @foreign_key_type Ecto.UUID

  schema "app_posts_communities" do
    belongs_to :post, App.Data.Post
    belongs_to :community, App.Data.Community
    belongs_to :membership, App.Data.UserCommunity, foreign_key: :users_communities_id
    field :last_synced_at, :utc_datetime_usec
    field :version, :integer, default: 1
    field :deleted_at, :utc_datetime_usec
    field :deleted_by_id, Ecto.UUID

    timestamps(type: :utc_datetime_usec)
  end

  @required [:post_id, :community_id, :users_communities_id]

  @doc """
  The rows above this one, by foreign key: deleting one copies its
  `deleted_at` here, see `App.Data.Deletes`.
  """
  def parents, do: [post_id: App.Data.Post, users_communities_id: App.Data.UserCommunity]

  @doc """
  The stream of the row's events: its post's, since where a post is
  visible is part of its history, see `App.Data.EventLog`.
  """
  def event_stream(row), do: {"app_posts", row.post_id}

  def changeset(row, params, _options \\ []) do
    row
    |> cast(params, @required)
    |> validate_required(@required)
    |> unique_constraint([:post_id, :community_id])
    |> put_change(:last_synced_at, DateTime.utc_now())
  end

  @doc """
  Removing the post from the community, or the leave cascade: a soft
  delete with the `deleted_at` the flow computed.
  """
  def remove_changeset(row, params, _options \\ []) do
    row
    |> cast(params, [:deleted_at, :deleted_by_id])
    |> validate_required([:deleted_at, :deleted_by_id])
    |> put_change(:last_synced_at, DateTime.utc_now())
  end

  @doc """
  Adding the post back: revives the row under the owner's current
  membership.
  """
  def revive_changeset(row, params, _options \\ []) do
    row
    |> cast(params, [:users_communities_id])
    |> validate_required([:users_communities_id])
    |> change(deleted_at: nil, deleted_by_id: nil)
    |> put_change(:last_synced_at, DateTime.utc_now())
  end
end
