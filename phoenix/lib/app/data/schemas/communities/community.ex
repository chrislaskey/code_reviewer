defmodule App.Data.Community do
  @moduledoc """
  A community: the soft boundary inside a deployment. Users join
  communities, see `App.Data.UserCommunity`, and a post is in the
  communities its owner chose. Syncs to its members.
  """

  use Ecto.Schema
  use App.Data.Synced
  import Ecto.Changeset

  @primary_key {:id, Ecto.UUID, autogenerate: [version: 7]}
  @foreign_key_type Ecto.UUID

  schema "app_communities" do
    field :name, :string
    field :last_synced_at, :utc_datetime_usec
    field :version, :integer, default: 1
    field :deleted_at, :utc_datetime_usec
    field :deleted_by_id, Ecto.UUID

    timestamps(type: :utc_datetime_usec)
  end

  def changeset(community, params, _options \\ []) do
    community
    |> cast(params, [:name])
    |> validate_required([:name])
    |> validate_length(:name, max: 80)
    |> put_change(:last_synced_at, DateTime.utc_now())
  end
end
