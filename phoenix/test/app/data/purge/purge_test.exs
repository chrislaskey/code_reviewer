defmodule App.Data.PurgeTest do
  use App.DataCase

  import App.Data.CommunitiesFixtures
  import App.Data.PostsFixtures
  import App.Data.UsersFixtures

  alias App.Data.Accounts
  alias App.Data.Memberships
  alias App.Data.Posting
  alias App.Data.Posts
  alias App.Data.PostsCommunities
  alias App.Data.Purge
  alias App.Data.Users
  alias App.Data.UsersCommunities

  defp later(days), do: DateTime.add(DateTime.utc_now(), days, :day)

  test "hard-deletes tombstones older than the window, children first, and records the horizon" do
    owner = user_scope_fixture(user_fixture())
    community = joined_community_fixture(owner)
    gone = post_fixture(owner, [community.id])
    kept = post_fixture(owner, [community.id])
    {:ok, _row} = Posting.remove_from_community(owner, gone, community.id)
    {:ok, _post} = Posting.delete(owner, gone)
    {:ok, _membership} = Memberships.leave(owner, community.id)

    assert Purge.horizon() == nil
    assert %{counts: %{"app_posts" => 0}} = Purge.run(now: later(89))
    assert Posts.get(gone.id, include_deleted: true)

    result = Purge.run(now: later(91))

    assert %{"app_posts" => 1, "app_posts_communities" => 2, "app_users_communities" => 1} =
             result.counts

    assert Posts.get(gone.id, include_deleted: true) == nil
    assert Posts.get(kept.id, include_deleted: true)
    assert PostsCommunities.list(filters: [post_id: kept.id], include_deleted: true) == []
    assert UsersCommunities.list(include_deleted: true) == []
    assert Purge.horizon() == result.horizon
    assert DateTime.compare(result.horizon, DateTime.utc_now()) == :gt
  end

  test "purges a deleted account, its app user row and its profile" do
    user = user_fixture()
    scope = user_scope_fixture(user)
    community = joined_community_fixture(scope)
    post_fixture(scope, [community.id])
    {:ok, _user} = Accounts.delete_user(user)

    assert %{counts: %{"users" => 1, "app_users" => 1, "app_user_profiles" => 1}} = Purge.run(now: later(91))

    assert App.Repo.get(App.Data.User, user.id) == nil
    assert App.Data.AppUsers.get(user.id, include_deleted: true) == nil
    assert Posts.list(filters: [owner_id: user.id], include_deleted: true) == []
    assert Users.get_by(email: user.email) == nil
  end
end
