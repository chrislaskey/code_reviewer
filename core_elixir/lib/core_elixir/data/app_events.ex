defmodule CoreElixir.Data.AppEvents do
  @moduledoc """
  The answers to pushed events: one row of `app_events` per event the
  server applied, by the event's id, so a retry gets the same answer.
  Written with the change it answers, in the same transaction.
  """
  use Ecto.Schema
  alias CoreElixir.Helpers.Time
  alias CoreElixir.Repo

  @primary_key {:id, Ecto.UUID, autogenerate: false}
  schema "app_events" do
    field :user_id, Ecto.UUID
    field :table_name, :string
    field :row_id, Ecto.UUID
    field :status, :string
    field :version, :integer
    field :conflicts, :map, default: %{}
    field :inserted_at, :utc_datetime_usec
  end

  @doc "Gets the answer recorded for the event, or nil."
  def get(id), do: Repo.get(__MODULE__, id)

  @doc "Records an event's answer."
  def create(answer) do
    Repo.insert!(%{answer | inserted_at: Time.now()})
  end
end
