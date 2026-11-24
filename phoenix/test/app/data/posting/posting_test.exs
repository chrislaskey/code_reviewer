defmodule App.Data.PostingTest do
  use App.DataCase

  import App.Data.CommunitiesFixtures
  import App.Data.PostsFixtures
  import App.Data.UsersFixtures

  alias App.Data.Events
  alias App.Data.Memberships
  alias App.Data.PostCommunity
  alias App.Data.Posting
  alias App.Data.Posts
  alias App.Data.PostsCommunities
  alias App.Web.Scope

  setup do
    owner = user_scope_fixture(user_fixture())
    reader = user_scope_fixture(user_fixture())

    [one, two, three] =
      for _ <- 1..3 do
        community = joined_community_fixture(owner)
        {:ok, _membership} = Memberships.join(reader, community.id)
        community
      end

    %{owner: owner, reader: reader, one: one, two: two, three: three}
  end

  defp feed_ids(scope, community \\ nil) do
    scope = if community, do: Scope.put_community(scope, community), else: scope
    scope |> Posting.feed() |> Enum.map(& &1.id)
  end

  describe "write/3" do
    test "puts the post in the chosen communities, with their owners preloaded in the feed",
         %{owner: owner, reader: reader, one: one, two: two, three: three} do
      post = post_fixture(owner, [one.id, two.id])

      assert feed_ids(reader, one) == [post.id]
      assert feed_ids(reader, two) == [post.id]
      assert feed_ids(reader, three) == []
      assert [%{owner: %{id: owner_id}}] = Posting.feed(Scope.put_community(reader, one))
      assert owner_id == owner.user.id
    end

    test "refuses a community the owner is not a member of", %{owner: owner} do
      other = community_fixture()

      assert {:error, :not_a_member} = Posting.write(owner, %{body: "hello"}, [other.id])
      assert Posts.list() == []
    end

    test "validates the post", %{owner: owner} do
      assert {:error, changeset} = Posting.write(owner, %{body: ""})
      assert %{body: ["can't be blank"]} = errors_on(changeset)
    end
  end

  describe "feed/1 across communities" do
    test "shows posts in shared communities and the user's own, and nothing else",
         %{owner: owner, reader: reader, one: one} do
      shared = post_fixture(owner, [one.id])
      alone = post_fixture(owner, [])

      stranger = user_scope_fixture(user_fixture())
      private = joined_community_fixture(stranger)
      unseen = post_fixture(stranger, [private.id])

      assert feed_ids(reader) == [shared.id]
      assert feed_ids(owner) == [alone.id, shared.id]
      assert feed_ids(stranger) == [unseen.id]
    end
  end

  describe "remove_from_community/3 and add_to_community/3" do
    test "removing a post from one community leaves it in the others",
         %{owner: owner, reader: reader, one: one, two: two} do
      post = post_fixture(owner, [one.id, two.id])

      assert {:ok, %PostCommunity{deleted_at: %DateTime{}}} =
               Posting.remove_from_community(owner, post, one.id)

      assert feed_ids(reader, one) == []
      assert feed_ids(reader, two) == [post.id]
    end

    test "adding a post back revives the same row", %{owner: owner, reader: reader, one: one} do
      post = post_fixture(owner, [one.id])
      [%{id: row_id}] = Posting.communities_of(owner, post)
      {:ok, _row} = Posting.remove_from_community(owner, post, one.id)

      assert {:ok, %PostCommunity{id: ^row_id, deleted_at: nil}} =
               Posting.add_to_community(owner, post, one.id)

      assert feed_ids(reader, one) == [post.id]
    end

    test "only the owner adds or removes", %{owner: owner, reader: reader, one: one, two: two} do
      post = post_fixture(owner, [one.id])

      assert {:error, :not_owner} = Posting.add_to_community(reader, post, two.id)
      assert {:error, :not_owner} = Posting.remove_from_community(reader, post, one.id)
      assert {:error, :not_owner} = Posting.delete(reader, post)
    end
  end

  describe "leaving and rejoining" do
    test "leaving removes the owner's posts from that community and rejoining brings them back",
         %{owner: owner, reader: reader, one: one, two: two} do
      post = post_fixture(owner, [one.id, two.id])
      other = post_fixture(reader, [one.id])

      {:ok, membership} = Memberships.leave(owner, one.id)

      assert feed_ids(reader, one) == [other.id]
      assert feed_ids(reader, two) == [post.id]

      [row] =
        PostsCommunities.list(
          filters: [post_id: post.id, community_id: one.id],
          include_deleted: true
        )

      assert row.deleted_at == membership.deleted_at

      {:ok, _membership} = Memberships.join(owner, one.id)
      assert feed_ids(reader, one) == [other.id, post.id]
    end

    test "a row the owner removed on purpose does not come back on rejoin",
         %{owner: owner, reader: reader, one: one} do
      kept = post_fixture(owner, [one.id])
      removed = post_fixture(owner, [one.id])
      {:ok, _row} = Posting.remove_from_community(owner, removed, one.id)

      {:ok, _membership} = Memberships.leave(owner, one.id)
      {:ok, _membership} = Memberships.join(owner, one.id)

      assert feed_ids(reader, one) == [kept.id]
    end
  end

  describe "delete/2" do
    test "hides the post from every feed and copies its deletion onto its community rows",
         %{owner: owner, reader: reader, one: one} do
      post = post_fixture(owner, [one.id])

      assert {:ok, %{deleted_at: %DateTime{} = deleted_at}} = Posting.delete(owner, post)

      assert feed_ids(reader, one) == []
      assert feed_ids(reader) == []
      assert feed_ids(owner) == []

      assert [%PostCommunity{deleted_at: ^deleted_at}] =
               PostsCommunities.list(filters: [post_id: post.id], include_deleted: true)
    end
  end

  describe "versions and events" do
    test "writing and deleting a post each record an event with the version it produced", %{
      owner: owner,
      one: one
    } do
      post = post_fixture(owner, [one.id], %{body: "hello"})
      assert post.version == 1

      {:ok, deleted} = Posting.delete(owner, post)
      assert deleted.version == 2

      assert [
               %{type: :create, base_version: 0, version: 1, changes: %{"body" => "hello"}},
               %{type: :update, base_version: 1, version: 2, changes: %{"deleted_at" => _}}
             ] = Events.list(filters: [row_table: "app_posts", row_id: post.id])
    end

    test "a write from a stale copy of a row is refused, and a delete from one merges like a device's",
         %{owner: owner, one: one} do
      post = post_fixture(owner, [one.id])
      {:ok, _edited} = Posts.update(post, %{body: "first"})

      assert {:error, changeset} = Posts.update(post, %{body: "from the stale copy"})
      assert changeset.errors[:version]
      assert Posts.get(post.id).body == "first"

      # An event from the stale copy: nobody else changed deleted_at
      assert {:ok, %{body: "first", deleted_at: %DateTime{}}} = Posting.delete(owner, post)
    end

    test "a device's edit of a field nobody else changed merges with a newer server version",
         %{owner: owner, one: one} do
      post = post_fixture(owner, [one.id], %{body: "hello"})

      event = fn base_version, changes ->
        %{
          id: Ecto.UUID.generate(version: 7),
          row_id: post.id,
          type: :update,
          base_version: base_version,
          base: %{"body" => "hello"},
          changes: changes
        }
      end

      {:ok, %{conflicts: %{}}, _post} = Posting.apply_event(owner, event.(1, %{"body" => "edited"}))

      {:ok, %{conflicts: conflicts, version: 2}, post} =
        Posting.apply_event(owner, event.(1, %{"body" => "mine"}))

      assert conflicts == %{"body" => %{"yours" => "mine", "server" => "edited"}}
      assert post.body == "edited"

      assert {:error, "not_editable: owner_id", _post} =
               Posting.apply_event(owner, event.(2, %{"owner_id" => Ecto.UUID.generate()}))
    end
  end
end
