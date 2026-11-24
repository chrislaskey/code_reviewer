defmodule App.Data.Sync.CommentsTest do
  @moduledoc """
  The product flows of `docs/design/comments.md`: comments on a post in a
  community, written by its members, moderated by the post's owner, and
  deleted with their thread. Written before the implementation.
  """

  use App.DataCase

  import App.Data.CommunitiesFixtures
  import App.Data.UsersFixtures
  import App.WebWrites

  alias App.Data.Memberships
  alias App.Data.Posting
  alias App.Data.Posts
  alias App.SyncClient

  setup do
    owner = user_scope_fixture(user_fixture())
    commenter = user_scope_fixture(user_fixture())
    reader = user_scope_fixture(user_fixture())
    stranger = user_scope_fixture(user_fixture())
    community = joined_community_fixture(owner)
    {:ok, _membership} = Memberships.join(commenter, community.id)
    {:ok, _membership} = Memberships.join(reader, community.id)
    post_id = web_post(owner, [community.id])

    %{
      owner: owner,
      commenter: commenter,
      reader: reader,
      stranger: stranger,
      community: community,
      post_id: post_id,
      thread: community_row_of(post_id, community.id)
    }
  end

  defp comment(scope, thread, body),
    do: create(scope, "app_comments", %{"posts_communities_id" => thread, "body" => body})

  defp remove(owner, post_id, community),
    do: Posting.remove_from_community(owner, Posts.get(post_id), community.id)

  defp add_back(owner, post_id, community),
    do: Posting.add_to_community(owner, Posts.get(post_id), community.id)

  describe "writing" do
    test "a member comments, and another member's device gets it with its author",
         %{commenter: commenter, reader: reader, thread: thread} do
      assert {%{"status" => "applied", "server" => %{"row" => %{"owner_id" => owner_id}}}, id} =
               comment(commenter, thread, "Nice")

      assert owner_id == commenter.user.id

      phone = SyncClient.pull(SyncClient.new(reader))

      assert %{"body" => "Nice", "posts_communities_id" => ^thread} =
               SyncClient.row(phone, "app_comments", id)

      assert SyncClient.row(phone, "app_users", commenter.user.id)
    end

    test "a comment that breaks the rules is refused at once with the form's errors",
         %{commenter: commenter, thread: thread} do
      assert {%{"status" => "rejected", "errors" => %{"body" => ["can't be blank"]}}, id} =
               comment(commenter, thread, " ")

      assert row("app_comments", id) == nil

      assert {%{"status" => "rejected", "errors" => %{"body" => ["should be at most 2000 character(s)"]}},
              _id} =
               comment(commenter, thread, String.duplicate("x", 2001))
    end

    test "only a live member comments, on a post in the community, and never as someone else",
         %{commenter: commenter, stranger: stranger, thread: thread} do
      assert {%{"status" => "rejected", "reason" => "not_a_member"}, _id} = comment(stranger, thread, "Hi")

      assert {%{"status" => "rejected", "reason" => "unknown_post_community"}, _id} =
               comment(commenter, uuid(), "Hi")

      assert {%{"status" => "rejected", "reason" => "unknown_post_community"}, _id} =
               create(commenter, "app_comments", %{"body" => "Hi"})

      assert {%{"status" => "rejected", "reason" => "not_editable: owner_id"}, _id} =
               create(commenter, "app_comments", %{
                 "posts_communities_id" => thread,
                 "body" => "Hi",
                 "owner_id" => stranger.user.id
               })
    end

    test "the commenter edits; the post's owner deletes but doesn't edit; another member does neither",
         %{owner: owner, commenter: commenter, reader: reader, thread: thread} do
      {_answer, id} = comment(commenter, thread, "First")

      assert %{"status" => "applied"} = update(commenter, "app_comments", id, %{"body" => "Edited"})

      assert %{"status" => "rejected", "reason" => "not_owner"} =
               update(owner, "app_comments", id, %{"body" => "Mine"})

      assert %{"status" => "rejected", "reason" => "not_owner"} = delete(reader, "app_comments", id)

      assert %{"status" => "applied"} = delete(owner, "app_comments", id)
      assert %{"body" => "Edited", "deleted_by_id" => deleted_by} = row("app_comments", id)
      assert deleted_by == owner.user.id
    end

    test "edge 3: the commenter can't bring back a comment the post's owner deleted, and can bring back their own",
         %{owner: owner, commenter: commenter, thread: thread} do
      {_answer, id} = comment(commenter, thread, "Hmm")

      delete(owner, "app_comments", id)
      assert %{"status" => "rejected", "reason" => "not_owner"} = undelete(commenter, "app_comments", id)
      assert %{"status" => "applied"} = undelete(owner, "app_comments", id)

      delete(commenter, "app_comments", id)
      assert %{"status" => "applied"} = undelete(commenter, "app_comments", id)
      assert deleted_at("app_comments", id) == nil
    end

    test "edge 1: a post in two communities has a thread in each, seen only by that community's members",
         %{owner: owner, commenter: commenter, reader: reader, community: community} do
      second = joined_community_fixture(owner)
      outsider = user_scope_fixture(user_fixture())
      {:ok, _membership} = Memberships.join(outsider, second.id)
      post_id = web_post(owner, [community.id, second.id])

      {_answer, here} = comment(commenter, community_row_of(post_id, community.id), "here")
      {_answer, there} = comment(outsider, community_row_of(post_id, second.id), "there")

      reader_phone = SyncClient.pull(SyncClient.new(reader))
      assert SyncClient.row(reader_phone, "app_comments", here)
      refute SyncClient.row(reader_phone, "app_comments", there)

      outsider_phone = SyncClient.pull(SyncClient.new(outsider))
      refute SyncClient.row(outsider_phone, "app_comments", here)
      assert SyncClient.row(outsider_phone, "app_comments", there)

      owner_phone = SyncClient.pull(SyncClient.new(owner))
      assert SyncClient.row(owner_phone, "app_comments", here)
      assert SyncClient.row(owner_phone, "app_comments", there)
    end

    test "edge 2: a commenter who leaves keeps their comment for the members, and can't change it until they rejoin",
         %{commenter: commenter, stranger: stranger, community: community, thread: thread} do
      {_answer, id} = comment(commenter, thread, "Stays")

      {:ok, _left} = Memberships.leave(commenter, community.id)
      assert deleted_at("app_comments", id) == nil

      assert %{"status" => "rejected", "reason" => "not_a_member"} =
               update(commenter, "app_comments", id, %{"body" => "Gone?"})

      assert %{"status" => "rejected", "reason" => "not_a_member"} = delete(commenter, "app_comments", id)

      # Someone who joins now still sees it, and who wrote it
      {:ok, _membership} = Memberships.join(stranger, community.id)
      phone = SyncClient.pull(SyncClient.new(stranger))
      assert SyncClient.row(phone, "app_comments", id)["body"] == "Stays"
      assert SyncClient.row(phone, "app_users", commenter.user.id)

      {:ok, _membership} = Memberships.join(commenter, community.id)
      assert %{"status" => "applied"} = update(commenter, "app_comments", id, %{"body" => "Back"})
    end
  end

  describe "deletes copy down" do
    test "edge 4: removing a post from a community takes its thread there, and adding it back brings it back but a comment deleted on its own",
         %{owner: owner, commenter: commenter, community: community, post_id: post_id, thread: thread} do
      {_answer, kept} = comment(commenter, thread, "Kept")
      {_answer, removed} = comment(commenter, thread, "Removed")
      delete(commenter, "app_comments", removed, DateTime.add(DateTime.utc_now(), -60, :second))
      removed_at = deleted_at("app_comments", removed)

      {:ok, _row} = remove(owner, post_id, community)
      assert deleted_at("app_comments", kept) == deleted_at("app_posts_communities", thread)
      assert deleted_at("app_comments", removed) == removed_at

      {:ok, _row} = add_back(owner, post_id, community)
      assert deleted_at("app_comments", kept) == nil
      assert deleted_at("app_comments", removed) == removed_at
    end

    test "edge 7: deleting a post takes every thread in every community, and undeleting brings them back",
         %{owner: owner, commenter: commenter, community: community} do
      second = joined_community_fixture(owner)
      {:ok, _membership} = Memberships.join(commenter, second.id)
      post_id = web_post(owner, [community.id, second.id])
      {_answer, here} = comment(commenter, community_row_of(post_id, community.id), "here")
      {_answer, there} = comment(commenter, community_row_of(post_id, second.id), "there")

      delete(owner, "app_posts", post_id)
      post_deleted_at = deleted_at("app_posts", post_id)
      assert deleted_at("app_comments", here) == post_deleted_at
      assert deleted_at("app_comments", there) == post_deleted_at

      undelete(owner, "app_posts", post_id)
      assert deleted_at("app_comments", here) == nil
      assert deleted_at("app_comments", there) == nil
    end

    test "the post's owner leaving the community takes the thread, and rejoining brings it back",
         %{owner: owner, commenter: commenter, community: community, thread: thread} do
      {_answer, id} = comment(commenter, thread, "Hi")

      {:ok, _left} = Memberships.leave(owner, community.id)
      assert deleted_at("app_comments", id) == deleted_at("app_posts_communities", thread)

      {:ok, _membership} = Memberships.join(owner, community.id)
      assert deleted_at("app_comments", id) == nil
    end

    test "edge 5: a comment written offline on a post removed from the community meanwhile is kept, deleted with the thread",
         %{owner: owner, commenter: commenter, community: community, post_id: post_id, thread: thread} do
      phone = SyncClient.pull(SyncClient.new(commenter))

      {phone, id} =
        SyncClient.create(phone, "app_comments", %{"posts_communities_id" => thread, "body" => "Offline"})

      {:ok, _row} = remove(owner, post_id, community)
      assert {_phone, %{"events" => [%{"status" => "applied"}]}} = SyncClient.push(phone)
      assert deleted_at("app_comments", id) == deleted_at("app_posts_communities", thread)

      {:ok, _row} = add_back(owner, post_id, community)
      assert deleted_at("app_comments", id) == nil
    end

    test "edge 6: a comment edited offline while the post's owner deleted it: the edit applies and it stays deleted",
         %{owner: owner, commenter: commenter, thread: thread} do
      {_answer, id} = comment(commenter, thread, "Draft")
      phone = SyncClient.pull(SyncClient.new(commenter))
      phone = SyncClient.change(phone, "app_comments", id, %{"body" => "Edited offline"})

      delete(owner, "app_comments", id)
      assert {_phone, %{"events" => [%{"status" => "applied"}]}} = SyncClient.push(phone)

      assert %{"body" => "Edited offline", "deleted_at" => at} = row("app_comments", id)
      assert at
    end
  end

  describe "the event log" do
    test "each comment is its own stream, and a copied delete is caused by the post's community row",
         %{owner: owner, commenter: commenter, community: community, post_id: post_id, thread: thread} do
      {_answer, id} = comment(commenter, thread, "First")
      update(commenter, "app_comments", id, %{"body" => "Second"})
      {:ok, _row} = remove(owner, post_id, community)
      thread_version = row("app_posts_communities", thread)["version"]

      assert [
               %{"sequence" => 1, "type" => "create", "row_id" => ^id},
               %{
                 "sequence" => 2,
                 "type" => "update",
                 "changes" => %{"body" => "Second"},
                 "caused_by_id" => nil
               },
               %{
                 "sequence" => 3,
                 "type" => "update",
                 "user_id" => user_id,
                 "caused_by_table" => "app_posts_communities",
                 "caused_by_id" => ^thread,
                 "caused_by_version" => ^thread_version
               }
             ] = stream("app_comments", id)

      assert user_id == owner.user.id
      refute Enum.any?(stream("app_posts", post_id), &(&1["row_table"] == "app_comments"))
    end
  end

  describe "seeing and syncing" do
    test "a member who joins later gets the threads with their authors, authors first",
         %{commenter: commenter, stranger: stranger, community: community, thread: thread} do
      {_answer, id} = comment(commenter, thread, "Hello")
      {:ok, _membership} = Memberships.join(stranger, community.id)

      phone = SyncClient.backfill(SyncClient.new(stranger), community.id)
      assert SyncClient.row(phone, "app_comments", id)
      assert SyncClient.row(phone, "app_users", commenter.user.id)

      keys =
        for page <- phone.last_pull, %{"table" => table, "row" => row} <- page["rows"], do: {table, row["id"]}

      at = fn key -> Enum.find_index(keys, &(&1 == key)) end
      assert at.({"app_users", commenter.user.id}) < at.({"app_comments", id})
      assert at.({"app_posts_communities", thread}) < at.({"app_comments", id})
    end

    test "a post added to another community reaches its members without the first community's thread",
         %{owner: owner, commenter: commenter, stranger: stranger, post_id: post_id, thread: thread} do
      {_answer, id} = comment(commenter, thread, "Here only")
      second = joined_community_fixture(owner)
      {:ok, _membership} = Memberships.join(stranger, second.id)
      phone = SyncClient.pull(SyncClient.new(stranger))

      {:ok, _row} = Posting.add_to_community(owner, Posts.get(post_id), second.id)
      phone = SyncClient.pull(phone)

      assert SyncClient.row(phone, "app_posts", post_id)
      refute SyncClient.row(phone, "app_comments", id)
    end

    test "the purge removes a deleted thread's comments and their streams",
         %{owner: owner, commenter: commenter, post_id: post_id, thread: thread} do
      {_answer, id} = comment(commenter, thread, "Gone")

      delete(owner, "app_posts", post_id)
      App.Data.Purge.run(now: DateTime.add(DateTime.utc_now(), 91, :day))

      assert row("app_comments", id) == nil
      assert stream("app_comments", id) == []
    end

    test "deleting an account deletes the user's comments", %{commenter: commenter, thread: thread} do
      {_answer, id} = comment(commenter, thread, "Bye")

      {:ok, _user} = App.Data.Accounts.delete_user(commenter.user)

      assert %{"deleted_at" => at, "deleted_by_id" => by} = row("app_comments", id)
      assert at && by == commenter.user.id
    end
  end
end
