defmodule App.Data.Visibility do
  @moduledoc """
  Which rows a user sees, as queries the data contexts' `visible_to`
  filters share, see `docs/design/multi-tenancy.md`.
  """

  import Ecto.Query, only: [from: 2]

  alias App.Data.PostCommunity
  alias App.Data.UserCommunity
  alias App.Web.Scope

  @doc """
  The ids of the posts visible to the scope's user, live or not, as a
  query for a subquery: the rows that hang off a post, its links and
  tags, are visible with it.
  """
  def post_ids(%Scope{user: user} = scope) do
    in_my_communities =
      from(pc in PostCommunity,
        where: pc.community_id in subquery(community_ids(scope)) and is_nil(pc.deleted_at),
        select: pc.post_id
      )

    from(post in App.Data.Post,
      where: post.owner_id == ^user.id or post.id in subquery(in_my_communities),
      select: post.id
    )
  end

  @doc """
  The ids of the communities the scope's user is a live member of.
  """
  def community_ids(%Scope{user: user}) do
    from(uc in UserCommunity,
      where: uc.user_id == ^user.id and is_nil(uc.deleted_at),
      select: uc.community_id
    )
  end

  @doc """
  The ids of the `posts_communities` rows, live or not, of the
  communities the scope's user is a live member of: the threads they
  see.
  """
  def post_community_ids(%Scope{} = scope) do
    from(pc in PostCommunity, where: pc.community_id in subquery(community_ids(scope)), select: pc.id)
  end

  @doc """
  The ids of the users who wrote a comment the scope's user sees, so a
  commenter who left still shows on their comments.
  """
  def commenter_ids(%Scope{} = scope) do
    from(c in App.Data.Comment,
      where: c.posts_communities_id in subquery(post_community_ids(scope)),
      select: c.owner_id
    )
  end
end
