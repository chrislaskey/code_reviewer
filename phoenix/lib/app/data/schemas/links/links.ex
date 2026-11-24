defmodule App.Data.Links do
  @moduledoc """
  The data context over `App.Data.Link`. Links are written through
  events, see `App.Data.Posting`.

  The `visible_to: scope` filter is what syncs to a user: the links of
  the posts they see, live or not.
  """

  use App.Data.Context, schema: App.Data.Link, sorts: [:inserted_at, :id]

  alias App.Data.Visibility
  alias App.Web.Scope

  def filter(query, :visible_to, %Scope{} = scope) do
    Ecto.Query.where(query, [link], link.post_id in subquery(Visibility.post_ids(scope)))
  end

  def filter(query, key, value), do: super(query, key, value)
end
