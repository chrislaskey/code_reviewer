defmodule App.Data.Memberships do
  @moduledoc """
  Joining, leaving and rejoining a community, over the
  `App.Data.UsersCommunities` context. A membership is a `users_communities`
  row of the scope's user.

  Leaving soft-deletes the row, and `App.Data.Deletes` copies its
  `deleted_at` onto the member's own posts in the community: every live
  `posts_communities` row added under the membership. Rejoining while the
  row still exists clears `deleted_at` on that same row, so its id is
  stable, and revives exactly the rows that left with it, unless their
  post has been deleted since. A row the owner removed on purpose was
  deleted at another moment and stays deleted. Once the purge has removed
  the membership, rejoining starts fresh.
  """

  alias App.Data.Communities
  alias App.Data.Deletes
  alias App.Data.UserCommunity
  alias App.Data.UsersCommunities
  alias App.Web.Scope

  @doc """
  The live membership of the scope's user in the community, or nil.
  """
  def membership(%Scope{user: user}, community_id) do
    UsersCommunities.get_by(user_id: user.id, community_id: community_id)
  end

  @doc """
  Whether the scope's user is a member of the community.
  """
  def member?(scope, community_id), do: membership(scope, community_id) != nil

  @doc """
  Joins the community, or rejoins it by reviving the membership left
  behind and what left with it. Returns the live membership either way.
  """
  def join(%Scope{user: user}, community_id) do
    community = Communities.get(community_id)

    case UsersCommunities.get_by([user_id: user.id, community_id: community_id], include_deleted: true) do
      _membership when community == nil ->
        {:error, :unknown_community}

      nil ->
        UsersCommunities.create(%{user_id: user.id, community_id: community_id})

      %UserCommunity{deleted_at: nil} = membership ->
        {:ok, membership}

      %UserCommunity{} = membership ->
        App.Repo.transact(fn -> rejoin(membership) end)
    end
  end

  defp rejoin(%UserCommunity{user_id: user_id} = left) do
    with {:ok, membership} <-
           UsersCommunities.update(left, %{}, changesets: &UserCommunity.rejoin_changeset/3) do
      :ok = Deletes.copy_down(left, membership, user_id)
      {:ok, membership}
    end
  end

  @doc """
  Leaves the community, taking the member's own posts there along.
  Returns `{:error, :not_found}` without a live membership.
  """
  def leave(%Scope{user: user} = scope, community_id) do
    case membership(scope, community_id) do
      nil ->
        {:error, :not_found}

      membership ->
        App.Repo.transact(fn -> leave_with_posts(membership, user.id) end)
    end
  end

  defp leave_with_posts(membership, user_id) do
    with {:ok, left} <-
           UsersCommunities.delete(membership,
             deleted_at: DateTime.utc_now(),
             deleted_by_id: user_id,
             changesets: &UserCommunity.leave_changeset/3
           ) do
      :ok = Deletes.copy_down(membership, left, user_id)
      {:ok, left}
    end
  end
end
