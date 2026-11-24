defmodule App.Data.Tag do
  @moduledoc """
  A tag, owned by the user who made it, like a post. Its name is unique
  per owner ignoring case among live tags, and it goes only on its
  owner's own posts, through `App.Data.PostTag`, see `App.Data.Tagging`
  and `docs/design/related-resources.md`.

  `changeset/3` is marked `@universal_changeset true`, see `App.Data.Post`.
  The name is trimmed before it, by the flow.
  """

  use Ecto.Schema
  use App.Data.Synced
  use App.Data.UniversalChangeset
  import Ecto.Changeset

  @primary_key {:id, Ecto.UUID, autogenerate: [version: 7]}
  @foreign_key_type Ecto.UUID

  schema "app_tags" do
    belongs_to :owner, App.Data.AppUser
    field :name, :string
    field :last_synced_at, :utc_datetime_usec
    field :version, :integer, default: 1
    field :deleted_at, :utc_datetime_usec
    field :deleted_by_id, Ecto.UUID

    timestamps(type: :utc_datetime_usec)
  end

  @doc """
  Making or renaming a tag. The owner is the `:owner_id` option, set by
  the flow, never a param.
  """
  @universal_changeset true
  def changeset(tag, params, options \\ []) do
    tag
    |> cast(params, [:name])
    |> put_owner(Keyword.get(options, :owner_id))
    |> validate_required([:owner_id, :name])
    |> validate_length(:name, max: 50)
    |> unique_constraint(:name, name: :app_tags_owner_id_lower_name_index)
    |> put_change(:last_synced_at, DateTime.utc_now())
  end

  defp put_owner(changeset, nil), do: changeset
  defp put_owner(changeset, owner_id), do: put_change(changeset, :owner_id, owner_id)

  @doc """
  The fields an `update` event may change, see `App.Data.EventFlow`.
  """
  def event_changeset(tag, params, options \\ []) do
    tag
    |> changeset(params, options)
    |> cast(params, [:deleted_at])
    |> App.Data.Synced.put_deleted_by(Keyword.get(options, :deleted_by_id))
  end

  @doc """
  The stream of the tag's events: its own, see `App.Data.EventLog`.
  """
  def event_stream(tag), do: {"app_tags", tag.id}
end
