defmodule App.Data.PostsCommunities do
  @moduledoc """
  The data context over `App.Data.PostCommunity`. The flows that write these rows are in `App.Data.Posting` and, for the
  leave cascade, `App.Data.Memberships`.

  The `visible_to: scope` filter is what syncs to a user: the rows of the
  communities they are a live member of, and the rows of their own posts,
  live or not.
  """

  use App.Data.Context, schema: App.Data.PostCommunity

  import Ecto.Query, only: [from: 2]

  alias App.Data.Post
  alias App.Data.UserCommunity
  alias App.Web.Scope

  def filter(query, :visible_to, %Scope{user: user}) do
    my_communities =
      from(uc in UserCommunity,
        where: uc.user_id == ^user.id and is_nil(uc.deleted_at),
        select: uc.community_id
      )

    my_posts = from(p in Post, where: p.owner_id == ^user.id, select: p.id)

    Ecto.Query.where(
      query,
      [pc],
      pc.community_id in subquery(my_communities) or pc.post_id in subquery(my_posts)
    )
  end

  def filter(query, key, value), do: super(query, key, value)
end
