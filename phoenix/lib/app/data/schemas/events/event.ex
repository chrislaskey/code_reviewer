defmodule App.Data.Event do
  @moduledoc """
  One change to a row of a resource that keeps a log, in its stream.
  Server-only and append-only, see `App.Data.EventLog` and "The event log"
  in `docs/design/related-resources.md`.
  """

  use Ecto.Schema
  import Ecto.Changeset

  @primary_key {:id, Ecto.UUID, autogenerate: [version: 7]}

  schema "app_events" do
    field :stream_table, :string
    field :stream_id, Ecto.UUID
    field :sequence, :integer
    field :row_table, :string
    field :row_id, Ecto.UUID
    field :user_id, Ecto.UUID
    field :type, Ecto.Enum, values: [:create, :update]
    field :base_version, :integer
    field :version, :integer
    field :changes, :map, default: %{}
    field :conflicts, :map, default: %{}
    field :caused_by_table, :string
    field :caused_by_id, Ecto.UUID
    field :caused_by_version, :integer

    timestamps(type: :utc_datetime_usec, updated_at: false)
  end

  @required [
    :stream_table,
    :stream_id,
    :sequence,
    :row_table,
    :row_id,
    :user_id,
    :type,
    :base_version,
    :version,
    :changes,
    :conflicts
  ]

  def changeset(event, params, _options \\ []) do
    event
    |> cast(params, [:id, :caused_by_table, :caused_by_id, :caused_by_version | @required])
    |> validate_required(@required)
    |> unique_constraint([:stream_table, :stream_id, :sequence])
  end
end
