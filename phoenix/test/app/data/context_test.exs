defmodule App.Data.ContextTest do
  use App.DataCase

  import App.Data.CommunitiesFixtures
  import App.Data.UsersFixtures

  alias App.Data.Accounts
  alias App.Data.Communities
  alias App.Data.UserTokens

  setup do
    %{user: user_fixture()}
  end

  describe "delete/2" do
    test "soft-deletes a schema with deleted_at, and the tombstone syncs", %{user: user} do
      community = community_fixture()

      assert {:ok, deleted} = Communities.delete(community, deleted_by_id: user.id)

      assert %DateTime{} = deleted.deleted_at
      assert deleted.deleted_by_id == user.id
      assert DateTime.compare(deleted.last_synced_at, community.last_synced_at) == :gt
      assert Communities.get(community.id) == nil
      assert Communities.get(community.id, include_deleted: true)
    end

    test "deletes by id, and refuses a row already soft-deleted", %{user: user} do
      community = community_fixture()

      assert {:ok, _deleted} = Communities.delete(community.id, deleted_by_id: user.id)
      assert {:error, :not_found} = Communities.delete(community.id, deleted_by_id: user.id)
    end

    test "with hard: true removes the row, soft-deleted or not", %{user: user} do
      live = community_fixture()
      tombstone = community_fixture()
      {:ok, _deleted} = Communities.delete(tombstone, deleted_by_id: user.id)

      assert {:ok, _deleted} = Communities.delete(live, hard: true)
      assert {:ok, _deleted} = Communities.delete(tombstone.id, hard: true)

      assert Communities.get(live.id, include_deleted: true) == nil
      assert Communities.get(tombstone.id, include_deleted: true) == nil
    end

    test "hard-deletes a schema without deleted_at", %{user: user} do
      Accounts.generate_user_session_token(user)
      [token] = UserTokens.list(filters: [user_id: user.id])

      assert {:ok, _deleted} = UserTokens.delete(token)
      assert UserTokens.list(filters: [user_id: user.id]) == []
    end
  end

  describe "delete_by/2" do
    test "soft-deletes the row found by the values", %{user: user} do
      community = community_fixture()

      assert {:ok, _deleted} = Communities.delete_by([name: community.name], deleted_by_id: user.id)
      assert Communities.get(community.id) == nil
      assert Communities.get(community.id, include_deleted: true)
    end
  end

  describe "delete_many/1" do
    test "soft-deletes every matching row", %{user: user} do
      community = community_fixture()

      assert {1, nil} = Communities.delete_many(filters: [id: community.id], deleted_by_id: user.id)

      assert %{deleted_by_id: deleted_by_id, deleted_at: %DateTime{}} =
               Communities.get(community.id, include_deleted: true)

      assert deleted_by_id == user.id
    end

    test "with hard: true removes the rows, soft-deleted or not", %{user: user} do
      community = community_fixture()
      {:ok, _deleted} = Communities.delete(community, deleted_by_id: user.id)

      assert {1, nil} = Communities.delete_many(filters: [id: community.id], hard: true)
      assert Communities.get(community.id, include_deleted: true) == nil
    end
  end

  describe "deleted_at_before and last_synced_at_before filters" do
    test "match tombstones older than the time, and nothing live", %{user: user} do
      live = community_fixture()
      tombstone = community_fixture()
      {:ok, _deleted} = Communities.delete(tombstone, deleted_by_id: user.id)
      later = DateTime.add(DateTime.utc_now(), 1, :second)
      filters = [deleted_at_before: later, last_synced_at_before: later]

      assert [%{id: id}] = Communities.list(filters: filters, include_deleted: true)
      assert id == tombstone.id

      too_early = [deleted_at_before: tombstone.inserted_at]
      assert Communities.list(filters: too_early, include_deleted: true) == []

      too_early = [last_synced_at_before: live.last_synced_at]
      assert Communities.list(filters: too_early, include_deleted: true) == []

      assert {1, nil} = Communities.delete_many(filters: filters, hard: true)
      assert Communities.get(live.id)
    end
  end
end
