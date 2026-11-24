defmodule App.Data.PostsTags do
  @moduledoc """
  The data context over `App.Data.PostTag`, in the order a post shows its
  tags: by position, then by name. Written through events, see
  `App.Data.Tagging`.

  The `visible_to: scope` filter is what syncs to a user: the rows of the
  posts they see, live or not.
  """

  use App.Data.Context, schema: App.Data.PostTag, sorts: [:position]

  alias App.Data.Visibility
  alias App.Web.Scope

  def filter(query, :visible_to, %Scope{} = scope) do
    Ecto.Query.where(query, [row], row.post_id in subquery(Visibility.post_ids(scope)))
  end

  def filter(query, key, value), do: super(query, key, value)

  # Ties on position go by the tag's name
  def sort(query, direction, :position) do
    query
    |> join(:tag)
    |> Ecto.Query.order_by([row, tag: tag], [
      {^direction, row.position},
      {^direction, fragment("lower(?)", tag.name)}
    ])
  end

  def sort(query, direction, field), do: super(query, direction, field)
end
