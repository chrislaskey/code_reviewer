defmodule App.Data.UserProfile do
  @moduledoc """
  A user's public details, one per user, created with their app user and
  edited only by them, see `App.Data.Profiles`. The reference for a
  single-owner resource without a server event log.

  `changeset/3` is marked `@universal_changeset true`, see
  `App.Data.Post`.
  """

  use Ecto.Schema
  use App.Data.Synced
  use App.Data.UniversalChangeset
  import Ecto.Changeset

  @primary_key {:id, Ecto.UUID, autogenerate: [version: 7]}
  @foreign_key_type Ecto.UUID

  schema "app_user_profiles" do
    belongs_to :user, App.Data.AppUser
    field :display_name, :string
    field :bio, :string
    field :last_synced_at, :utc_datetime_usec
    field :version, :integer, default: 1
    field :deleted_at, :utc_datetime_usec
    field :deleted_by_id, Ecto.UUID

    timestamps(type: :utc_datetime_usec)
  end

  @doc """
  Writing or editing a profile. The user is the `:user_id` option, set by
  the flow, never a param.
  """
  @universal_changeset true
  def changeset(profile, params, options \\ []) do
    profile
    |> cast(params, [:display_name, :bio])
    |> put_user(Keyword.get(options, :user_id))
    |> validate_required([:user_id])
    |> validate_length(:display_name, max: 100)
    |> validate_length(:bio, max: 1000)
    |> put_change(:last_synced_at, DateTime.utc_now())
  end

  defp put_user(changeset, nil), do: changeset
  defp put_user(changeset, user_id), do: put_change(changeset, :user_id, user_id)

  @doc """
  Account deletion: a soft delete.
  """
  def delete_changeset(profile, params, _options \\ []) do
    profile
    |> cast(params, [:deleted_at, :deleted_by_id])
    |> validate_required([:deleted_at, :deleted_by_id])
    |> put_change(:last_synced_at, DateTime.utc_now())
  end
end
