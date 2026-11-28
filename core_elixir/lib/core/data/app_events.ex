defmodule Core.Data.AppEvents do
  @moduledoc """
  The events the server answered: one row of `app_events` per pushed
  event it applied, by the event's id, with its answer, so a retry gets
  the same answer. The Rust server's table: written with the change it
  answers, in the same transaction.
  """
  use Ecto.Schema
  alias Core.Repo

  @primary_key {:id, Ecto.UUID, autogenerate: false}
  schema "app_events" do
    field :user_id, Ecto.UUID
    field :table_name, :string
    field :row_id, Ecto.UUID
    field :type, :string
    field :base_version, :integer
    field :base, :map
    field :changes, :map
    field :made_at, :utc_datetime_usec
    field :status, :string
    field :conflicts, :map
    field :version, :integer
    field :last_error, :string
    field :attempts, :integer, default: 0
  end

  @doc "Gets the event the server answered, or nil."
  def get(id), do: Repo.get(__MODULE__, id)

  @doc "Records an answered event."
  def create(event), do: Repo.insert!(event)
end
