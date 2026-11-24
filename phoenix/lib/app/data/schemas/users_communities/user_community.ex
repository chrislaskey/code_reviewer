defmodule App.Data.UserCommunity do
  @moduledoc """
  A membership: a user's relationship to a community. Leaving
  soft-deletes the row and rejoining revives the same row, see
  `App.Data.Memberships`, so the unique index on the pair is plain. `role` is `member` for every row until permissions exist.

  Syncs to clients. A membership a client pushes goes through the join
  and leave flows, never on its timestamp alone: the server wins.
  """

  use Ecto.Schema
  use App.Data.Synced
  import Ecto.Changeset

  @primary_key {:id, Ecto.UUID, autogenerate: [version: 7]}
  @foreign_key_type Ecto.UUID

  schema "app_users_communities" do
    belongs_to :user, App.Data.AppUser
    belongs_to :community, App.Data.Community
    field :role, Ecto.Enum, values: [:member], default: :member
    field :last_synced_at, :utc_datetime_usec
    field :version, :integer, default: 1
    field :deleted_at, :utc_datetime_usec
    field :deleted_by_id, Ecto.UUID

    timestamps(type: :utc_datetime_usec)
  end

  @required [:user_id, :community_id]
  @optional [:role]

  def changeset(membership, params, _options \\ []) do
    membership
    |> cast(params, @required ++ @optional)
    |> validate_required(@required)
    |> unique_constraint([:user_id, :community_id])
    |> put_change(:last_synced_at, DateTime.utc_now())
  end

  @doc """
  Leaving: soft-deletes the membership with the `deleted_at` the leave
  flow computed once, so the rows that leave with it share the value.
  """
  def leave_changeset(membership, params, _options \\ []) do
    membership
    |> cast(params, [:deleted_at, :deleted_by_id])
    |> validate_required([:deleted_at, :deleted_by_id])
    |> put_change(:last_synced_at, DateTime.utc_now())
  end

  @doc """
  Rejoining: revives the soft-deleted membership. Reads no params.
  """
  def rejoin_changeset(membership, _params \\ %{}, _options \\ []) do
    membership
    |> change(deleted_at: nil, deleted_by_id: nil)
    |> put_change(:last_synced_at, DateTime.utc_now())
  end
end
