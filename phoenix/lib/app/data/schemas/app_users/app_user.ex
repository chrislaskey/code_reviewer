defmodule App.Data.AppUser do
  @moduledoc """
  A user as the app knows them: the `app_users` table, one row per user
  with the same id as their `users` row, written when they register, see
  `App.Data.Accounts.register_user/1`. Every foreign key of an `app_`
  table points here, never at `users`, which holds credentials and stays
  on the server.

  The email is a copy for display, kept in step by `App.Data.Accounts`.
  The row syncs to clients for display, and is never accepted from them.
  """

  use Ecto.Schema
  use App.Data.Synced
  import Ecto.Changeset

  # The id is the user's, see the moduledoc.
  @primary_key {:id, Ecto.UUID, autogenerate: false}
  @foreign_key_type Ecto.UUID

  schema "app_users" do
    field :email, :string
    field :joined_at, :utc_datetime_usec
    field :last_synced_at, :utc_datetime_usec
    field :version, :integer, default: 1
    field :deleted_at, :utc_datetime_usec
    field :deleted_by_id, Ecto.UUID

    timestamps(type: :utc_datetime_usec)
  end

  @required [:id, :email, :joined_at]

  def changeset(user, params, _options \\ []) do
    user
    |> cast(params, @required)
    |> validate_required(@required)
    |> unique_constraint(:id, name: :app_users_id_index)
    |> put_change(:last_synced_at, DateTime.utc_now())
  end

  @doc """
  Account deletion: a soft delete.
  """
  def delete_changeset(user, params, _options \\ []) do
    user
    |> cast(params, [:deleted_at, :deleted_by_id])
    |> validate_required([:deleted_at, :deleted_by_id])
    |> put_change(:last_synced_at, DateTime.utc_now())
  end
end
