defmodule App.Data.Sync.RelatedResourcesTest do
  @moduledoc """
  The product flows of `docs/design/related-resources.md`: links, tags
  and posts_tags, deletes that copy onto the rows below, and the shared
  event log. Written before the implementation, so a failure is either a
  bug or a question about the spec.

  The web and the devices write the same way: the web builds the
  messages a device would and pushes them at once, see `App.WebWrites`;
  a device queues them in an `App.SyncClient`.
  """

  use App.DataCase

  import App.Data.CommunitiesFixtures
  import App.Data.UsersFixtures
  import App.WebWrites

  alias App.Data.Memberships
  alias App.SyncClient

  setup do
    owner = user_scope_fixture(user_fixture())
    member = user_scope_fixture(user_fixture())
    stranger = user_scope_fixture(user_fixture())
    community = joined_community_fixture(owner)
    {:ok, _membership} = Memberships.join(member, community.id)
    %{owner: owner, member: member, stranger: stranger, community: community}
  end

  describe "links" do
    test "the owner adds, edits and removes a link from the web, and a member's device follows",
         %{owner: owner, member: member, community: community} do
      post_id = web_post(owner, [community.id])
      phone = SyncClient.pull(SyncClient.new(member))

      {%{"status" => "applied", "version" => 1}, link_id} = create(owner, "app_links", link(post_id))

      assert %{"status" => "applied", "version" => 2} =
               update(owner, "app_links", link_id, %{"label" => "Hex"})

      phone = SyncClient.pull(phone)

      assert %{"label" => "Hex", "url" => "https://hexdocs.pm", "post_id" => ^post_id, "deleted_at" => nil} =
               SyncClient.row(phone, "app_links", link_id)

      assert %{"status" => "applied"} = delete(owner, "app_links", link_id)
      phone = SyncClient.pull(phone)

      assert SyncClient.row(phone, "app_links", link_id)["deleted_at"]
      assert deleted_at("app_posts", post_id) == nil
    end

    test "a link that breaks the rules is refused at once with the form's errors", %{owner: owner} do
      post_id = web_post(owner)

      {answer, link_id} =
        create(owner, "app_links", %{"post_id" => post_id, "label" => "", "url" => "ftp://example.com"})

      assert %{"status" => "rejected", "reason" => "invalid: " <> _, "errors" => errors} = answer
      assert errors == %{"label" => ["can't be blank"], "url" => ["must be an http:// or https:// URL"]}
      assert row("app_links", link_id) == nil

      {answer, _id} =
        create(owner, "app_links", %{
          "post_id" => post_id,
          "label" => String.duplicate("x", 101),
          "url" => "https://"
        })

      assert answer["errors"] == %{
               "label" => ["should be at most 100 character(s)"],
               "url" => ["must be an http:// or https:// URL"]
             }
    end

    test "only the post's owner adds and edits its links, on a post that exists, and never moves one",
         %{owner: owner, member: member, community: community} do
      post_id = web_post(owner, [community.id])

      assert {%{"status" => "rejected", "reason" => "not_owner"}, _id} =
               create(member, "app_links", link(post_id))

      {%{"status" => "applied"}, link_id} = create(owner, "app_links", link(post_id))

      assert %{"status" => "rejected", "reason" => "not_owner"} =
               update(member, "app_links", link_id, %{"label" => "Mine"})

      assert {%{"status" => "rejected", "reason" => "unknown_post"}, _id} =
               create(owner, "app_links", link(uuid()))

      assert %{"status" => "rejected", "reason" => "not_editable: post_id"} =
               update(owner, "app_links", link_id, %{"post_id" => web_post(owner)})
    end

    test "edge 1: an offline edit to a link of a post deleted meanwhile applies, and shows when the post comes back",
         %{owner: owner} do
      post_id = web_post(owner)
      {_answer, link_id} = create(owner, "app_links", link(post_id))
      phone = SyncClient.pull(SyncClient.new(owner))
      phone = SyncClient.change(phone, "app_links", link_id, %{"label" => "Edited offline"})

      assert %{"status" => "applied"} = delete(owner, "app_posts", post_id)
      assert {_phone, %{"events" => [%{"status" => "applied"}]}} = SyncClient.push(phone)

      assert %{"label" => "Edited offline", "deleted_at" => at} = row("app_links", link_id)
      assert at == deleted_at("app_posts", post_id)

      assert %{"status" => "applied"} = undelete(owner, "app_posts", post_id)
      assert %{"label" => "Edited offline", "deleted_at" => nil} = row("app_links", link_id)
    end

    test "edge 2: a link added offline to a post deleted meanwhile is kept, deleted with the post",
         %{owner: owner} do
      post_id = web_post(owner)
      phone = SyncClient.pull(SyncClient.new(owner))
      {phone, link_id} = SyncClient.create(phone, "app_links", link(post_id))

      delete(owner, "app_posts", post_id)
      assert {_phone, %{"events" => [%{"status" => "applied"}]}} = SyncClient.push(phone)
      assert deleted_at("app_links", link_id) == deleted_at("app_posts", post_id)

      undelete(owner, "app_posts", post_id)
      assert deleted_at("app_links", link_id) == nil
    end

    test "edge 3: a link removed before its post was deleted stays removed when the post comes back",
         %{owner: owner, community: community} do
      post_id = web_post(owner, [community.id])
      {_answer, kept} = create(owner, "app_links", link(post_id, "Kept"))
      {_answer, removed} = create(owner, "app_links", link(post_id, "Removed"))

      delete(owner, "app_links", removed, DateTime.add(DateTime.utc_now(), -60, :second))
      removed_at = deleted_at("app_links", removed)

      delete(owner, "app_posts", post_id)
      post_deleted_at = deleted_at("app_posts", post_id)
      assert deleted_at("app_links", kept) == post_deleted_at
      assert deleted_at("app_links", removed) == removed_at

      undelete(owner, "app_posts", post_id)
      assert deleted_at("app_links", kept) == nil
      assert deleted_at("app_posts_communities", community_row_of(post_id)) == nil
      assert deleted_at("app_links", removed) == removed_at
    end
  end

  describe "tags" do
    test "a tag is created, renamed and deleted by its owner, with names unique per owner ignoring case",
         %{owner: owner, member: member} do
      {%{"status" => "applied", "server" => %{"row" => %{"owner_id" => owner_id}}}, tag_id} =
        create(owner, "app_tags", %{"name" => "  Elixir "})

      assert owner_id == owner.user.id
      assert row("app_tags", tag_id)["name"] == "Elixir"

      assert {%{"status" => "rejected", "errors" => %{"name" => ["has already been taken"]}}, _id} =
               create(owner, "app_tags", %{"name" => "elixir"})

      assert {%{"status" => "applied"}, _id} = create(member, "app_tags", %{"name" => "elixir"})

      assert {%{"status" => "rejected", "reason" => "not_editable: owner_id"}, _id} =
               create(owner, "app_tags", %{"name" => "As them", "owner_id" => member.user.id})

      assert %{"status" => "applied"} = update(owner, "app_tags", tag_id, %{"name" => "Erlang"})

      assert %{"status" => "rejected", "reason" => "not_owner"} =
               update(member, "app_tags", tag_id, %{"name" => "Theirs"})

      assert %{"status" => "applied"} = delete(owner, "app_tags", tag_id)

      # A deleted tag's name is free again
      assert {%{"status" => "applied"}, _id} = create(owner, "app_tags", %{"name" => "erlang"})
    end

    test "edge 4: one person's two devices create the same name offline, and the second is refused with what it tagged",
         %{owner: owner} do
      post_id = web_post(owner)
      phone = SyncClient.pull(SyncClient.new(owner))
      laptop = SyncClient.pull(SyncClient.new(owner))

      {phone, phone_tag} = SyncClient.create(phone, "app_tags", %{"name" => "Elixir"})
      {laptop, laptop_tag} = SyncClient.create(laptop, "app_tags", %{"name" => "elixir"})
      {laptop, laptop_tagging} = SyncClient.create(laptop, "app_posts_tags", tagging(post_id, laptop_tag))

      assert {_phone, %{"events" => [%{"status" => "applied"}]}} = SyncClient.push(phone)
      {laptop, %{"events" => [tag_answer, tagging_answer]}} = SyncClient.push(laptop)

      assert %{"status" => "rejected", "errors" => %{"name" => ["has already been taken"]}} = tag_answer
      assert %{"status" => "rejected", "reason" => "unknown_tag"} = tagging_answer
      refute SyncClient.row(laptop, "app_tags", laptop_tag)
      refute SyncClient.row(laptop, "app_posts_tags", laptop_tagging)

      assert [%{"id" => ^phone_tag}] = laptop |> SyncClient.pull() |> SyncClient.rows("app_tags")
    end

    test "a tag goes only on its owner's own posts", %{owner: owner, member: member, community: community} do
      mine = web_post(owner, [community.id])
      theirs = web_post(member, [community.id])
      my_tag = tag(owner, "mine")
      their_tag = tag(member, "theirs")

      assert {%{"status" => "rejected", "reason" => "not_owner"}, _id} =
               create(owner, "app_posts_tags", tagging(theirs, my_tag))

      assert {%{"status" => "rejected", "reason" => "not_own_tag"}, _id} =
               create(owner, "app_posts_tags", tagging(mine, their_tag))

      assert {%{"status" => "rejected", "reason" => "unknown_tag"}, _id} =
               create(owner, "app_posts_tags", tagging(mine, uuid()))

      assert {%{"status" => "applied"}, _id} = create(owner, "app_posts_tags", tagging(mine, my_tag))
    end

    test "edge 10: the same tag renamed on two devices offline is a conflict, like a post's body",
         %{owner: owner} do
      tag_id = tag(owner, "draft")
      phone = SyncClient.pull(SyncClient.new(owner))
      laptop = SyncClient.pull(SyncClient.new(owner))
      phone = SyncClient.change(phone, "app_tags", tag_id, %{"name" => "phone"})
      laptop = SyncClient.change(laptop, "app_tags", tag_id, %{"name" => "laptop"})

      assert {_phone, %{"events" => [%{"status" => "applied"}]}} = SyncClient.push(phone)

      assert {laptop,
              %{
                "events" => [
                  %{
                    "status" => "conflict",
                    "conflicts" => %{"name" => %{"yours" => "laptop", "server" => "phone"}}
                  }
                ]
              }} = SyncClient.push(laptop)

      assert [_conflict] = laptop.conflicts
      assert row("app_tags", tag_id)["name"] == "phone"
    end

    test "edge 11: a member sees a tag only on a post in a community they are in, and sees renames",
         %{owner: owner, member: member, stranger: stranger, community: community} do
      shown_on = web_post(owner, [community.id])
      private = web_post(owner)
      shown = tag(owner, "shown")
      private_tag = tag(owner, "private")
      _unused = tag(owner, "unused")
      create(owner, "app_posts_tags", tagging(shown_on, shown))
      create(owner, "app_posts_tags", tagging(private, private_tag))

      phone = SyncClient.pull(SyncClient.new(member))
      assert Enum.map(SyncClient.rows(phone, "app_tags"), & &1["id"]) == [shown]

      update(owner, "app_tags", shown, %{"name" => "renamed"})
      assert SyncClient.row(SyncClient.pull(phone), "app_tags", shown)["name"] == "renamed"

      assert SyncClient.rows(SyncClient.pull(SyncClient.new(stranger)), "app_tags") == []
      assert length(SyncClient.rows(SyncClient.pull(SyncClient.new(owner)), "app_tags")) == 3
    end
  end

  describe "posts_tags" do
    test "the owner tags a post with a position and a note and edits them, and a member's device follows",
         %{owner: owner, member: member, community: community} do
      post_id = web_post(owner, [community.id])
      alpha = tag(owner, "alpha")
      beta = tag(owner, "beta")

      {%{"status" => "applied"}, first} = create(owner, "app_posts_tags", tagging(post_id, beta, 0, "first"))
      {%{"status" => "applied"}, second} = create(owner, "app_posts_tags", tagging(post_id, alpha, 1))

      assert %{"status" => "applied"} = update(owner, "app_posts_tags", first, %{"note" => "still first"})

      assert {%{"status" => "rejected", "errors" => %{"note" => ["should be at most 200 character(s)"]}}, _id} =
               create(owner, "app_posts_tags", tagging(web_post(owner), alpha, 0, String.duplicate("x", 201)))

      assert %{"status" => "rejected", "reason" => "not_editable: tag_id"} =
               update(owner, "app_posts_tags", second, %{"tag_id" => beta})

      phone = SyncClient.pull(SyncClient.new(member))

      assert %{"position" => 0, "note" => "still first", "tag_id" => ^beta} =
               SyncClient.row(phone, "app_posts_tags", first)

      assert %{"position" => 1, "note" => nil} = SyncClient.row(phone, "app_posts_tags", second)
    end

    test "edge 7: untagging and tagging again revives the same row, and a create for the pair is answered with it",
         %{owner: owner} do
      post_id = web_post(owner)
      tag_id = tag(owner, "elixir")
      {_answer, tagging_id} = create(owner, "app_posts_tags", tagging(post_id, tag_id, 0, "old"))
      delete(owner, "app_posts_tags", tagging_id)

      assert {%{"status" => "rejected", "reason" => "exists", "server" => %{"row" => %{"id" => ^tagging_id}}},
              other_id} = create(owner, "app_posts_tags", tagging(post_id, tag_id, 3, "new"))

      assert row("app_posts_tags", other_id) == nil

      assert %{"status" => "applied"} =
               update(owner, "app_posts_tags", tagging_id, %{
                 "deleted_at" => nil,
                 "position" => 3,
                 "note" => "new"
               })

      assert %{"deleted_at" => nil, "position" => 3, "note" => "new"} = row("app_posts_tags", tagging_id)
    end

    test "edge 5: two devices tag the same post with the same tag, and the second keeps the server's row",
         %{owner: owner} do
      post_id = web_post(owner)
      tag_id = tag(owner, "elixir")
      phone = SyncClient.pull(SyncClient.new(owner))
      laptop = SyncClient.pull(SyncClient.new(owner))
      {phone, phone_tagging} = SyncClient.create(phone, "app_posts_tags", tagging(post_id, tag_id))
      {laptop, _laptop_tagging} = SyncClient.create(laptop, "app_posts_tags", tagging(post_id, tag_id, 1))

      assert {_phone, %{"events" => [%{"status" => "applied"}]}} = SyncClient.push(phone)

      assert {laptop, %{"events" => [%{"status" => "rejected", "reason" => "exists"}]}} =
               SyncClient.push(laptop)

      assert [%{"id" => ^phone_tagging, "position" => 0}] = SyncClient.rows(laptop, "app_posts_tags")
    end

    test "edge 6: tags placed at the same position on two devices both apply, and tie", %{owner: owner} do
      post_id = web_post(owner)
      [a, b, c, d] = for name <- ~w(a b c d), do: tag(owner, name)
      {_answer, on_a} = create(owner, "app_posts_tags", tagging(post_id, a, 0))
      {_answer, on_b} = create(owner, "app_posts_tags", tagging(post_id, b, 1))
      {_answer, on_c} = create(owner, "app_posts_tags", tagging(post_id, c, 2))
      phone = SyncClient.pull(SyncClient.new(owner))
      laptop = SyncClient.pull(SyncClient.new(owner))

      # The phone moves c to the front; the laptop adds d at the front
      phone = SyncClient.change(phone, "app_posts_tags", on_c, %{"position" => 0})
      phone = SyncClient.change(phone, "app_posts_tags", on_a, %{"position" => 1})
      phone = SyncClient.change(phone, "app_posts_tags", on_b, %{"position" => 2})
      {laptop, on_d} = SyncClient.create(laptop, "app_posts_tags", tagging(post_id, d, 0))

      assert {_phone, %{"events" => answers}} = SyncClient.push(phone)
      assert Enum.all?(answers, &(&1["status"] == "applied"))
      assert {_laptop, %{"events" => [%{"status" => "applied"}]}} = SyncClient.push(laptop)

      assert row("app_posts_tags", on_c)["position"] == 0
      assert row("app_posts_tags", on_d)["position"] == 0
    end
  end

  describe "deletes copy down" do
    test "deleting a post copies its deleted_at to its links, its tags on it and its community rows, and leaves the tags",
         %{owner: owner, member: member, community: community} do
      post_id = web_post(owner, [community.id])
      {_answer, link_id} = create(owner, "app_links", link(post_id))
      tag_id = tag(owner, "elixir")
      {_answer, tagging_id} = create(owner, "app_posts_tags", tagging(post_id, tag_id))
      community_row_id = community_row_of(post_id)
      owner_phone = SyncClient.pull(SyncClient.new(owner))
      member_phone = SyncClient.pull(SyncClient.new(member))

      assert %{"status" => "applied"} = delete(owner, "app_posts", post_id)
      %{"deleted_at" => at, "deleted_by_id" => by} = row("app_posts", post_id)
      assert at && by == owner.user.id

      below = [
        {"app_links", link_id},
        {"app_posts_tags", tagging_id},
        {"app_posts_communities", community_row_id}
      ]

      for {table, id} <- below do
        assert %{"deleted_at" => ^at, "deleted_by_id" => ^by, "version" => 2} = row(table, id)
      end

      assert deleted_at("app_tags", tag_id) == nil

      owner_phone = SyncClient.pull(owner_phone)
      for {table, id} <- below, do: assert(SyncClient.row(owner_phone, table, id)["deleted_at"] == at)

      # A member's device learns from the community row's tombstone
      member_phone = SyncClient.pull(member_phone)
      assert SyncClient.row(member_phone, "app_posts_communities", community_row_id)["deleted_at"] == at
    end

    test "deleting a tag removes it from posts and leaves the posts, and undeleting puts it back",
         %{owner: owner} do
      post_id = web_post(owner)
      tag_id = tag(owner, "elixir")
      {_answer, tagging_id} = create(owner, "app_posts_tags", tagging(post_id, tag_id))

      delete(owner, "app_tags", tag_id)
      assert deleted_at("app_posts_tags", tagging_id) == deleted_at("app_tags", tag_id)
      assert deleted_at("app_posts", post_id) == nil

      undelete(owner, "app_tags", tag_id)
      assert deleted_at("app_posts_tags", tagging_id) == nil
    end

    test "edge 8: with its post and its tag both deleted, a tagging comes back only once both are, in either order",
         %{owner: owner} do
      post_id = web_post(owner)
      tag_id = tag(owner, "elixir")
      {_answer, tagging_id} = create(owner, "app_posts_tags", tagging(post_id, tag_id))

      # The tag comes back first
      delete(owner, "app_posts", post_id)
      delete(owner, "app_tags", tag_id)
      assert deleted_at("app_posts_tags", tagging_id) == deleted_at("app_posts", post_id)

      undelete(owner, "app_tags", tag_id)
      assert deleted_at("app_posts_tags", tagging_id) == deleted_at("app_posts", post_id)

      undelete(owner, "app_posts", post_id)
      assert deleted_at("app_posts_tags", tagging_id) == nil

      # The post comes back first: the tagging waits with the tag's deleted_at
      delete(owner, "app_posts", post_id)
      delete(owner, "app_tags", tag_id)

      undelete(owner, "app_posts", post_id)
      assert deleted_at("app_posts_tags", tagging_id) == deleted_at("app_tags", tag_id)

      undelete(owner, "app_tags", tag_id)
      assert deleted_at("app_posts_tags", tagging_id) == nil
    end

    test "edge 9: a post and its owner's membership both deleted, the post's community row comes back only once both are",
         %{owner: owner, community: community} do
      post_id = web_post(owner, [community.id])
      community_row_id = community_row_of(post_id)

      # Deleted, then left: undeleting waits for the rejoin
      delete(owner, "app_posts", post_id)
      {:ok, _membership} = Memberships.leave(owner, community.id)
      undelete(owner, "app_posts", post_id)

      assert deleted_at("app_posts_communities", community_row_id) ==
               deleted_at("app_users_communities", membership_id(owner, community))

      {:ok, _membership} = Memberships.join(owner, community.id)
      assert deleted_at("app_posts_communities", community_row_id) == nil

      # Left, then deleted: rejoining waits for the undelete
      {:ok, _membership} = Memberships.leave(owner, community.id)
      delete(owner, "app_posts", post_id)
      {:ok, _membership} = Memberships.join(owner, community.id)
      assert deleted_at("app_posts_communities", community_row_id) == deleted_at("app_posts", post_id)

      undelete(owner, "app_posts", post_id)
      assert deleted_at("app_posts_communities", community_row_id) == nil
    end
  end

  describe "the event log" do
    test "a post's stream holds the post, its community rows, its links and its tags, in order",
         %{owner: owner, community: community} do
      post_id = web_post(owner, [community.id])
      {_answer, link_id} = create(owner, "app_links", link(post_id))
      tag_id = tag(owner, "elixir")
      {_answer, tagging_id} = create(owner, "app_posts_tags", tagging(post_id, tag_id))
      update(owner, "app_links", link_id, %{"label" => "Hex"})

      assert [
               %{
                 "sequence" => 1,
                 "row_table" => "app_posts",
                 "type" => "create",
                 "changes" => %{"body" => "a post"}
               },
               %{"sequence" => 2, "row_table" => "app_posts_communities", "type" => "create"},
               %{"sequence" => 3, "row_table" => "app_links", "row_id" => ^link_id, "type" => "create"},
               %{
                 "sequence" => 4,
                 "row_table" => "app_posts_tags",
                 "row_id" => ^tagging_id,
                 "type" => "create"
               },
               %{
                 "sequence" => 5,
                 "row_table" => "app_links",
                 "type" => "update",
                 "changes" => %{"label" => "Hex"},
                 "base_version" => 1,
                 "version" => 2,
                 "caused_by_id" => nil
               }
             ] = stream("app_posts", post_id)

      assert [%{"sequence" => 1, "row_table" => "app_tags", "row_id" => ^tag_id, "type" => "create"}] =
               stream("app_tags", tag_id)
    end

    test "deleting a post writes an event for every row it copies to, caused by the post",
         %{owner: owner, community: community} do
      post_id = web_post(owner, [community.id])
      create(owner, "app_links", link(post_id))
      {_answer, tagging_id} = create(owner, "app_posts_tags", tagging(post_id, tag(owner, "elixir")))

      delete(owner, "app_posts", post_id)
      %{"deleted_at" => at, "version" => post_version} = row("app_posts", post_id)

      [deleted | copies] = Enum.drop(stream("app_posts", post_id), 4)

      assert %{"row_table" => "app_posts", "changes" => %{"deleted_at" => ^at}, "caused_by_id" => nil} =
               deleted

      assert Enum.sort(Enum.map(copies, & &1["row_table"])) ==
               ~w(app_links app_posts_communities app_posts_tags)

      for copy <- copies do
        assert %{
                 "type" => "update",
                 "user_id" => user_id,
                 "caused_by_table" => "app_posts",
                 "caused_by_id" => ^post_id,
                 "caused_by_version" => ^post_version,
                 "changes" => %{"deleted_at" => ^at}
               } = copy

        assert user_id == owner.user.id
      end

      assert Enum.any?(copies, &(&1["row_id"] == tagging_id))
    end

    test "deleting a tag logs in the tag's stream, and its copies in the post's, caused by the tag",
         %{owner: owner} do
      post_id = web_post(owner)
      tag_id = tag(owner, "elixir")
      {_answer, tagging_id} = create(owner, "app_posts_tags", tagging(post_id, tag_id))

      delete(owner, "app_tags", tag_id)
      tag_version = row("app_tags", tag_id)["version"]

      assert [%{"type" => "create"}, %{"type" => "update", "changes" => %{"deleted_at" => _at}}] =
               stream("app_tags", tag_id)

      assert %{
               "row_table" => "app_posts_tags",
               "row_id" => ^tagging_id,
               "caused_by_table" => "app_tags",
               "caused_by_id" => ^tag_id,
               "caused_by_version" => ^tag_version
             } = List.last(stream("app_posts", post_id))
    end

    test "leaving a community writes an event into each of the member's posts there, caused by the membership",
         %{owner: owner, community: community} do
      post_id = web_post(owner, [community.id])
      membership_id = Memberships.membership(owner, community.id).id

      {:ok, left} = Memberships.leave(owner, community.id)

      assert %{
               "row_table" => "app_posts_communities",
               "type" => "update",
               "caused_by_table" => "app_users_communities",
               "caused_by_id" => ^membership_id,
               "caused_by_version" => version
             } = List.last(stream("app_posts", post_id))

      assert version == left.version

      {:ok, _membership} = Memberships.join(owner, community.id)

      assert %{"changes" => %{"deleted_at" => nil}, "caused_by_id" => ^membership_id} =
               List.last(stream("app_posts", post_id))
    end

    test "a retried link event gets the recorded answer and applies once", %{owner: owner} do
      post_id = web_post(owner)
      {_answer, link_id} = create(owner, "app_links", link(post_id))
      phone = SyncClient.pull(SyncClient.new(owner))
      phone = SyncClient.change(phone, "app_links", link_id, %{"label" => "Once"})
      [event] = phone.events

      assert %{"events" => [%{"status" => "applied", "version" => 2}]} = web(owner, [event])
      assert %{"events" => [%{"status" => "applied", "version" => 2}]} = web(owner, [event])
      assert row("app_links", link_id)["version"] == 2
      assert Enum.count(stream("app_posts", post_id), &(&1["id"] == event["id"])) == 1
    end

    test "profiles change by version and log nothing", %{owner: owner} do
      phone = SyncClient.pull(SyncClient.new(owner))
      [profile] = SyncClient.rows(phone, "app_user_profiles")
      phone = SyncClient.change(phone, "app_user_profiles", profile["id"], %{"bio" => "Hello"})

      assert {_phone, %{"events" => [%{"status" => "applied"}]}} = SyncClient.push(phone)

      assert %{rows: [[0]]} =
               App.Repo.query!("SELECT count(*) FROM app_events WHERE row_table = 'app_user_profiles'")
    end

    test "the web gets a conflict at once when a device saved the same field first", %{owner: owner} do
      post_id = web_post(owner)
      {_answer, link_id} = create(owner, "app_links", link(post_id, "Loaded"))
      loaded = row("app_links", link_id)

      phone = SyncClient.pull(SyncClient.new(owner))
      phone = SyncClient.change(phone, "app_links", link_id, %{"label" => "Phone"})
      assert {_phone, %{"events" => [%{"status" => "applied"}]}} = SyncClient.push(phone)

      saved =
        event("app_links", link_id, "update", loaded["version"], %{"label" => "Loaded"}, %{"label" => "Web"})

      assert %{
               "events" => [
                 %{
                   "status" => "conflict",
                   "conflicts" => %{"label" => %{"yours" => "Web", "server" => "Phone"}}
                 }
               ]
             } = web(owner, [saved])
    end
  end

  describe "seeing and syncing" do
    test "a post added to a community reaches a member's device with its links and tags, parents first",
         %{owner: owner, member: member, community: community} do
      post_id = web_post(owner)
      {_answer, link_id} = create(owner, "app_links", link(post_id))
      tag_id = tag(owner, "elixir")
      {_answer, tagging_id} = create(owner, "app_posts_tags", tagging(post_id, tag_id))

      phone = SyncClient.pull(SyncClient.new(member))
      refute SyncClient.row(phone, "app_posts", post_id)

      web(owner, [], [
        %{"table" => "app_posts_communities", "row" => community_row(owner, post_id, community.id)}
      ])

      phone = SyncClient.pull(phone)

      assert SyncClient.row(phone, "app_links", link_id)
      assert SyncClient.row(phone, "app_tags", tag_id)["name"] == "elixir"
      assert SyncClient.row(phone, "app_posts_tags", tagging_id)

      keys =
        for page <- phone.last_pull, %{"table" => table, "row" => row} <- page["rows"], do: {table, row["id"]}

      at = fn key -> Enum.find_index(keys, &(&1 == key)) end
      assert at.({"app_posts", post_id}) < at.({"app_links", link_id})
      assert at.({"app_posts", post_id}) < at.({"app_posts_tags", tagging_id})
      assert at.({"app_tags", tag_id}) < at.({"app_posts_tags", tagging_id})
    end

    test "a backfill carries a post's links and tags too", %{
      owner: owner,
      stranger: stranger,
      community: community
    } do
      post_id = web_post(owner, [community.id])
      {_answer, link_id} = create(owner, "app_links", link(post_id))
      tag_id = tag(owner, "elixir")
      {_answer, tagging_id} = create(owner, "app_posts_tags", tagging(post_id, tag_id))

      {:ok, _membership} = Memberships.join(stranger, community.id)
      phone = SyncClient.backfill(SyncClient.new(stranger), community.id)

      assert SyncClient.row(phone, "app_links", link_id)
      assert SyncClient.row(phone, "app_tags", tag_id)
      assert SyncClient.row(phone, "app_posts_tags", tagging_id)
    end

    test "the purge removes a deleted post with what it copied to, and its stream, and keeps the tag",
         %{owner: owner, community: community} do
      post_id = web_post(owner, [community.id])
      {_answer, link_id} = create(owner, "app_links", link(post_id))
      tag_id = tag(owner, "elixir")
      {_answer, tagging_id} = create(owner, "app_posts_tags", tagging(post_id, tag_id))
      community_row_id = community_row_of(post_id)

      delete(owner, "app_posts", post_id)
      App.Data.Purge.run(now: DateTime.add(DateTime.utc_now(), 91, :day))

      for {table, id} <- [
            {"app_posts", post_id},
            {"app_links", link_id},
            {"app_posts_tags", tagging_id},
            {"app_posts_communities", community_row_id}
          ],
          do: assert(row(table, id) == nil)

      assert stream("app_posts", post_id) == []
      assert row("app_tags", tag_id)
      assert [_created] = stream("app_tags", tag_id)
    end
  end
end
