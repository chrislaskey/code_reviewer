defmodule App.Data.MembershipsTest do
  use App.DataCase

  import App.Data.CommunitiesFixtures
  import App.Data.UsersFixtures

  alias App.Data.Memberships
  alias App.Data.UserCommunity
  alias App.Data.UsersCommunities

  setup do
    user = user_fixture()
    scope = user_scope_fixture(user)
    %{scope: scope, user: user, community: community_fixture()}
  end

  describe "join/2" do
    test "creates the membership", %{scope: scope, user: user, community: community} do
      refute Memberships.member?(scope, community.id)

      assert {:ok, %UserCommunity{} = membership} = Memberships.join(scope, community.id)
      assert membership.user_id == user.id
      assert membership.community_id == community.id
      assert membership.role == :member
      assert Memberships.member?(scope, community.id)
    end

    test "is a no-op for a member", %{scope: scope, community: community} do
      {:ok, %{id: id}} = Memberships.join(scope, community.id)
      assert {:ok, %{id: ^id}} = Memberships.join(scope, community.id)
      assert [_] = UsersCommunities.list()
    end
  end

  describe "join/2 of a community that is not there" do
    test "refuses an unknown or deleted community", %{scope: scope} do
      assert Memberships.join(scope, Ecto.UUID.generate(version: 7)) == {:error, :unknown_community}
    end
  end

  describe "leave/2" do
    test "soft-deletes the membership, which is then invisible everywhere",
         %{scope: scope, user: user, community: community} do
      {:ok, %{id: id}} = Memberships.join(scope, community.id)

      assert {:ok, %UserCommunity{id: ^id} = left} = Memberships.leave(scope, community.id)
      assert left.deleted_at
      assert left.deleted_by_id == user.id

      refute Memberships.member?(scope, community.id)
      refute Memberships.membership(scope, community.id)
      assert UsersCommunities.list() == []
      assert UsersCommunities.get(id) == nil
      assert %UserCommunity{} = UsersCommunities.get(id, include_deleted: true)
    end

    test "returns not found without a membership", %{scope: scope, community: community} do
      assert Memberships.leave(scope, community.id) == {:error, :not_found}
    end
  end

  describe "rejoining" do
    test "restores the same membership row", %{scope: scope, community: community} do
      {:ok, %{id: id}} = Memberships.join(scope, community.id)
      {:ok, _left} = Memberships.leave(scope, community.id)

      assert {:ok, %UserCommunity{id: ^id, deleted_at: nil, deleted_by_id: nil}} =
               Memberships.join(scope, community.id)

      assert Memberships.member?(scope, community.id)
      assert [%{id: ^id}] = UsersCommunities.list()
    end
  end
end
