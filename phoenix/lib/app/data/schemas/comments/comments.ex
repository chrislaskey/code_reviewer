defmodule App.Data.Comments do
  @moduledoc """
  The data context over `App.Data.Comment`, oldest first. Written through
  events, see `App.Data.Commenting`.

  The `visible_to: scope` filter is what syncs to a user: the comments in
  the communities they are a live member of, live or not.
  """

  use App.Data.Context, schema: App.Data.Comment, sorts: [:inserted_at, :id]

  alias App.Data.Visibility
  alias App.Web.Scope

  def filter(query, :visible_to, %Scope{} = scope) do
    Ecto.Query.where(
      query,
      [comment],
      comment.posts_communities_id in subquery(Visibility.post_community_ids(scope))
    )
  end

  def filter(query, key, value), do: super(query, key, value)
end
