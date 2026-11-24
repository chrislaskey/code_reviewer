defmodule App.Data.Tags do
  @moduledoc """
  The data context over `App.Data.Tag`, by name. Written through events,
  see `App.Data.Tagging`.

  The `visible_to: scope` filter is what syncs to a user: their own tags,
  and the tags on the posts they see.
  """

  use App.Data.Context, schema: App.Data.Tag, sorts: [:name]

  import Ecto.Query, only: [from: 2]

  alias App.Data.PostTag
  alias App.Data.Visibility
  alias App.Web.Scope

  def filter(query, :visible_to, %Scope{user: user} = scope) do
    on_visible_posts =
      from(pt in PostTag, where: pt.post_id in subquery(Visibility.post_ids(scope)), select: pt.tag_id)

    Ecto.Query.where(query, [tag], tag.owner_id == ^user.id or tag.id in subquery(on_visible_posts))
  end

  def filter(query, key, value), do: super(query, key, value)

  def sort(query, direction, :name) do
    Ecto.Query.order_by(query, [tag], [{^direction, fragment("lower(?)", tag.name)}])
  end

  def sort(query, direction, field), do: super(query, direction, field)
end
