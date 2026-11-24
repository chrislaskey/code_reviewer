defmodule App.Data.SyncTest do
  use App.DataCase

  import App.Data.CommunitiesFixtures
  import App.Data.PostsFixtures
  import App.Data.UsersFixtures

  alias App.Data.Events
  alias App.Data.Memberships
  alias App.Data.Posting
  alias App.Data.Posts
  alias App.Data.Profiles
  alias App.Data.Sync
  alias App.Data.Synced
  alias App.SyncClient

  setup do
    owner = user_scope_fixture(user_fixture())
    other = user_scope_fixture(user_fixture())
    community = joined_community_fixture(owner)
    {:ok, _membership} = Memberships.join(other, community.id)
    %{owner: owner, other: other, community: community}
  end

  # A post written on a device, in the community: the post's create event
  # and its posts_communities row, queued together.
  defp write_post(client, body, community_id, opts \\ []) do
    {client, post_id} = SyncClient.create(client, "app_posts", %{"body" => body}, opts)

    # The membership as the device knows it, which may be stale
    user_id = client.scope.user.id

    membership_id =
      case Enum.find(
             SyncClient.rows(client, "app_users_communities"),
             &(&1["user_id"] == user_id and &1["community_id"] == community_id)
           ) do
        %{"id" => id} -> id
        nil -> Memberships.membership(client.scope, community_id).id
      end

    client =
      SyncClient.write(
        client,
        "app_posts_communities",
        %{"post_id" => post_id, "community_id" => community_id, "users_communities_id" => membership_id},
        opts
      )

    {client, post_id}
  end

  # An edit made on the server, as the web app or another device would,
  # from the version the server has
  defp edit_on_server(scope, post, changes) do
    Posting.apply_event(scope, %{
      id: Ecto.UUID.generate(version: 7),
      row_id: post.id,
      type: :update,
      base_version: Posts.get(post.id, include_deleted: true).version,
      base: %{},
      changes: changes
    })
  end

  test "every synced schema is marked and listed, parents first" do
    for schema <- Sync.schemas(), do: assert(Synced.synced?(schema))

    assert Enum.map(Sync.tables(), &elem(&1, 0)) ==
             ~w(app_users app_user_profiles app_communities app_users_communities app_posts app_posts_communities
                app_links app_tags app_posts_tags app_comments)
  end

  describe "pull/2" do
    test "delivers what the user can see, across tables, in cursor order with a page token",
         %{owner: owner, other: other, community: community} do
      post_fixture(other, [community.id])
      post_fixture(owner, [community.id])
      post_fixture(owner, [])

      phone = SyncClient.pull(SyncClient.new(owner), limit: 3)

      assert length(phone.last_pull) > 1

      assert Enum.map(SyncClient.rows(phone, "app_users"), & &1["id"]) ==
               Enum.sort([owner.user.id, other.user.id])

      assert [%{"id" => community_id}] = SyncClient.rows(phone, "app_communities")
      assert community_id == community.id
      assert length(SyncClient.rows(phone, "app_users_communities")) == 2
      assert length(SyncClient.rows(phone, "app_posts")) == 3
      assert length(SyncClient.rows(phone, "app_posts_communities")) == 2
      assert %{"last_synced_at" => <<_::binary>>, "id" => <<_::binary>>} = phone.cursor

      # Every page ends later than the one before, and the last one has no
      # next page
      cursors = for page <- phone.last_pull, do: {page["cursor"]["last_synced_at"], page["cursor"]["id"]}
      assert cursors == Enum.sort(cursors) and cursors == Enum.uniq(cursors)
      assert List.last(phone.last_pull)["next"] == nil
    end

    test "the overlap buffer re-delivers without duplicating", %{owner: owner, community: community} do
      post = post_fixture(owner, [community.id])
      phone = SyncClient.pull(SyncClient.new(owner))
      assert SyncClient.row(phone, "app_posts", post.id)

      again = SyncClient.pull(phone)

      assert [page] = again.last_pull
      assert post.id in for(%{"row" => row} <- page["rows"], do: row["id"])
      assert again.rows == phone.rows
      assert again.cursor == phone.cursor
    end

    test "a posts_communities row for a newly joined community arrives with its post and owner",
         %{owner: owner, other: other, community: community} do
      phone = SyncClient.pull(SyncClient.new(owner))
      old = post_fixture(other, [])
      phone = SyncClient.pull(phone)
      refute SyncClient.row(phone, "app_posts", old.id)

      {:ok, _row} = Posting.add_to_community(other, old, community.id)
      phone = SyncClient.pull(phone)

      assert SyncClient.row(phone, "app_posts", old.id)["body"] == old.body
      assert SyncClient.row(phone, "app_users", other.user.id)

      # The owner and the post come before the row that carries them, once
      [page] = phone.last_pull
      keys = for %{"table" => table, "row" => row} <- page["rows"], do: {table, row["id"]}
      [row] = for {"app_posts_communities", id} <- keys, do: id
      assert keys == Enum.uniq(keys)

      assert Enum.find_index(keys, &(&1 == {"app_users", other.user.id})) <
               Enum.find_index(keys, &(&1 == {"app_posts", old.id}))

      assert Enum.find_index(keys, &(&1 == {"app_posts", old.id})) <
               Enum.find_index(keys, &(&1 == {"app_posts_communities", row}))
    end

    test "a post removed from a community and a left membership arrive as deletes",
         %{owner: owner, other: other, community: community} do
      post = post_fixture(other, [community.id])
      phone = SyncClient.pull(SyncClient.new(owner))

      {:ok, _row} = Posting.remove_from_community(other, post, community.id)
      phone = SyncClient.pull(phone)
      [row] = for row <- SyncClient.rows(phone, "app_posts_communities"), row["post_id"] == post.id, do: row
      assert row["deleted_at"]

      {:ok, membership} = Memberships.leave(owner, community.id)
      phone = SyncClient.pull(phone)
      assert SyncClient.row(phone, "app_users_communities", membership.id)["deleted_at"]
    end
  end

  describe "push/2" do
    test "a post is created by an event, and an edit from the version the server has applies",
         %{owner: owner, community: community} do
      {phone, post_id} = write_post(SyncClient.new(owner), "draft", community.id)

      {phone,
       %{
         "events" => [%{"row_id" => ^post_id, "status" => "applied", "version" => 1}],
         "rejected" => [%{"table" => "app_posts_communities", "reason" => "server_wins"}]
       }} = SyncClient.push(phone)

      assert SyncClient.row(phone, "app_posts", post_id)["version"] == 1

      phone = SyncClient.change(phone, "app_posts", post_id, %{"body" => "edited"})
      {phone, %{"events" => [%{"status" => "applied", "version" => 2}]}} = SyncClient.push(phone)

      assert Posts.get(post_id).body == "edited"
      assert SyncClient.row(phone, "app_posts", post_id)["version"] == 2

      assert [%{type: :create, base_version: 0, version: 1}, %{type: :update, base_version: 1, version: 2}] =
               Events.list(filters: [row_table: "app_posts", row_id: post_id])
    end

    test "the same field edited on two devices keeps the edit that reached the server first, and the other becomes a conflict",
         %{owner: owner, community: community} do
      post = post_fixture(owner, [community.id], %{body: "draft"})
      phone = SyncClient.pull(SyncClient.new(owner))
      laptop = SyncClient.pull(SyncClient.new(owner))

      # The phone is offline all along; the laptop edits and syncs first
      phone = SyncClient.change(phone, "app_posts", post.id, %{"body" => "from the phone"})
      laptop = SyncClient.change(laptop, "app_posts", post.id, %{"body" => "from the laptop"})
      {_laptop, %{"events" => [%{"status" => "applied", "version" => 2}]}} = SyncClient.push(laptop)

      {phone, %{"events" => [answer]}} = SyncClient.push(phone)

      assert %{
               "status" => "conflict",
               "version" => 2,
               "conflicts" => %{"body" => %{"yours" => "from the phone", "server" => "from the laptop"}}
             } = answer

      assert Posts.get(post.id).body == "from the laptop"
      assert SyncClient.row(phone, "app_posts", post.id)["body"] == "from the laptop"
      assert [%{"conflicts" => %{"body" => _}}] = phone.conflicts

      # The rejected change is in the post's history with nothing applied
      assert %{changes: changes, conflicts: %{"body" => _}, version: 2} =
               Events.get(answer["id"])

      assert changes == %{}
    end

    test "a conflict reapplied from the server's version wins, and one ignored changes nothing",
         %{owner: owner, community: community} do
      post = post_fixture(owner, [community.id], %{body: "draft"})
      phone = SyncClient.pull(SyncClient.new(owner))

      {:ok, _event, _post} = edit_on_server(owner, post, %{"body" => "on the server"})
      phone = SyncClient.change(phone, "app_posts", post.id, %{"body" => "mine"})
      {phone, _response} = SyncClient.push(phone)
      [conflict] = phone.conflicts

      ignored = SyncClient.resolve(phone, conflict["id"], :ignore)
      assert ignored.conflicts == [] and ignored.events == []

      phone = SyncClient.resolve(phone, conflict["id"], :reapply)
      assert [%{"base_version" => 2, "base" => %{"body" => "on the server"}}] = phone.events
      {phone, %{"events" => [%{"status" => "applied", "version" => 3}]}} = SyncClient.push(phone)

      assert Posts.get(post.id).body == "mine"
      assert phone.conflicts == []
    end

    test "different fields edited on two devices both apply", %{owner: owner, community: community} do
      post = post_fixture(owner, [community.id], %{body: "draft"})
      phone = SyncClient.pull(SyncClient.new(owner))

      {:ok, _event, _post} = edit_on_server(owner, post, %{"body" => "edited on the laptop"})

      deleted_at = DateTime.to_iso8601(DateTime.utc_now())
      phone = SyncClient.change(phone, "app_posts", post.id, %{"deleted_at" => deleted_at})
      {_phone, %{"events" => [answer]}} = SyncClient.push(phone)

      assert %{"status" => "applied", "version" => 3, "conflicts" => %{}} = answer
      deleted = Posts.get(post.id, include_deleted: true)
      assert deleted.body == "edited on the laptop"
      assert DateTime.to_iso8601(deleted.deleted_at) == deleted_at
      assert deleted.deleted_by_id == owner.user.id
    end

    test "edits made one after another offline do not conflict with each other",
         %{owner: owner, community: community} do
      post = post_fixture(owner, [community.id], %{body: "draft"})
      phone = SyncClient.pull(SyncClient.new(owner))

      phone = SyncClient.change(phone, "app_posts", post.id, %{"body" => "first"})
      {phone, %{"events" => [%{"status" => "applied"}]}} = SyncClient.push(phone)

      # Queued from version 1 again, before the push answered
      phone = %{phone | rows: put_in(phone.rows, ["app_posts", post.id, "version"], 1)}
      phone = SyncClient.change(phone, "app_posts", post.id, %{"body" => "second"})
      phone = SyncClient.change(phone, "app_posts", post.id, %{"body" => "third"})
      {phone, %{"events" => answers}} = SyncClient.push(phone)

      assert Enum.map(answers, & &1["status"]) == ["applied", "applied"]
      assert Posts.get(post.id).body == "third"
      assert phone.conflicts == []
    end

    test "a retried event gets the same answer and applies nothing twice",
         %{owner: owner, other: other, community: community} do
      post = post_fixture(owner, [community.id], %{body: "draft"})
      phone = SyncClient.pull(SyncClient.new(owner))
      phone = SyncClient.change(phone, "app_posts", post.id, %{"body" => "once"})
      [event] = phone.events

      assert %{"events" => [%{"status" => "applied", "version" => 2}]} = wire(Sync.push(owner, [], [event]))
      assert %{"events" => [%{"status" => "applied", "version" => 2}]} = wire(Sync.push(owner, [], [event]))
      assert Posts.get(post.id).version == 2
      assert length(Events.list(filters: [row_table: "app_posts", row_id: post.id])) == 2

      assert %{"events" => [%{"status" => "rejected", "reason" => "event_id_taken"}]} =
               wire(Sync.push(other, [], [event]))
    end

    test "a future-dated deletion in an event is bounded to server time", %{
      owner: owner,
      community: community
    } do
      post = post_fixture(owner, [community.id])
      phone = SyncClient.pull(SyncClient.new(owner, clock_offset: 24 * 60 * 60))
      future = DateTime.utc_now() |> DateTime.add(1, :day) |> DateTime.to_iso8601()
      phone = SyncClient.change(phone, "app_posts", post.id, %{"deleted_at" => future})

      {_phone, %{"events" => [%{"status" => "applied"}]}} = SyncClient.push(phone)

      deleted = Posts.get(post.id, include_deleted: true)
      assert DateTime.compare(deleted.deleted_at, DateTime.utc_now()) != :gt
    end

    test "a row pushed for a community the user left is rejected", %{owner: owner, community: community} do
      phone = SyncClient.pull(SyncClient.new(owner))
      {:ok, _membership} = Memberships.leave(owner, community.id)

      {phone, post_id} = write_post(phone, "too late", community.id)
      {phone, response} = SyncClient.push(phone)

      assert [%{"row_id" => ^post_id, "status" => "applied"}] = response["events"]

      assert [%{"table" => "app_posts_communities", "reason" => "not_a_member", "server" => nil}] =
               response["rejected"]

      assert Posting.feed(App.Web.Scope.put_community(owner, community)) == []
      assert SyncClient.rows(phone, "app_posts_communities") == []
    end

    test "memberships and community rows are server-wins", %{owner: owner, community: community} do
      phone = SyncClient.pull(SyncClient.new(owner))

      [membership] =
        Enum.filter(SyncClient.rows(phone, "app_users_communities"), &(&1["user_id"] == owner.user.id))

      phone = SyncClient.delete(phone, "app_users_communities", membership["id"])
      {phone, %{"rejected" => [%{"reason" => "server_wins", "server" => server}]}} = SyncClient.push(phone)

      assert server["row"]["id"] == membership["id"]
      assert server["row"]["deleted_at"]
      refute Memberships.member?(owner, community.id)
      assert SyncClient.row(phone, "app_users_communities", membership["id"])["deleted_at"]

      phone =
        SyncClient.write(phone, "app_users_communities", %{"id" => membership["id"], "deleted_at" => nil})

      {_phone, %{"rejected" => [%{"reason" => "server_wins", "server" => server}]}} = SyncClient.push(phone)
      assert server["row"]["deleted_at"] == nil
      assert Memberships.member?(owner, community.id)
    end

    test "a datetime in the wrong format is rejected, in a row and in an event", %{owner: owner} do
      row = %{"id" => Ecto.UUID.generate(version: 7), "name" => "x", "updated_at" => "2026-10-25T22:31:55Z"}

      assert %{"rejected" => [%{"reason" => "bad_datetime: updated_at"}]} =
               Sync.push(owner, [%{"table" => "app_communities", "row" => row}])

      phone = SyncClient.new(owner)
      {phone, _id} = SyncClient.create(phone, "app_posts", %{"body" => "x"})
      [event] = phone.events
      event = %{event | "type" => "update", "changes" => %{"deleted_at" => "2026-10-25T22:31:55Z"}}

      assert %{"events" => [%{"status" => "rejected", "reason" => "bad_datetime: deleted_at"}]} =
               Sync.push(owner, [], [event])
    end

    test "another user's post, a post as a row, and an app user row are refused",
         %{owner: owner, other: other} do
      theirs = post_fixture(other, [])
      now = DateTime.to_iso8601(DateTime.utc_now())

      event = %{
        "id" => Ecto.UUID.generate(version: 7),
        "table" => "app_posts",
        "row_id" => theirs.id,
        "type" => "update",
        "base_version" => theirs.version,
        "base" => %{"body" => theirs.body},
        "changes" => %{"body" => "mine now"}
      }

      assert %{
               "events" => [
                 %{
                   "status" => "rejected",
                   "reason" => "not_owner",
                   "server" => %{"row" => %{"body" => body}}
                 }
               ]
             } =
               wire(Sync.push(owner, [], [event]))

      assert body == theirs.body

      row = %{"id" => theirs.id, "body" => "mine now", "owner_id" => owner.user.id, "updated_at" => now}

      assert %{"rejected" => [%{"reason" => "events_only"}]} =
               wire(Sync.push(owner, [%{"table" => "app_posts", "row" => row}]))

      assert Posts.get(theirs.id).body == theirs.body

      assert %{"rejected" => [%{"reason" => "server_wins", "server" => %{"row" => %{"email" => email}}}]} =
               wire(
                 Sync.push(owner, [
                   %{
                     "table" => "app_users",
                     "row" => %{
                       "id" => owner.user.id,
                       "email" => "new@example.com",
                       "joined_at" => now,
                       "updated_at" => now
                     }
                   }
                 ])
               )

      assert email == owner.user.email
    end

    # As JSON would carry it
    defp wire(term), do: term |> Jason.encode!() |> Jason.decode!()
  end

  describe "the lifecycle" do
    test "a device of another member receives an account deletion through sync",
         %{owner: owner, other: other, community: community} do
      post = post_fixture(owner, [community.id])
      phone = SyncClient.pull(SyncClient.new(other))
      assert SyncClient.row(phone, "app_posts", post.id)

      [membership] =
        Enum.filter(SyncClient.rows(phone, "app_users_communities"), &(&1["user_id"] == owner.user.id))

      {:ok, _user} = App.Data.Accounts.delete_user(owner.user)
      phone = SyncClient.pull(phone)

      assert SyncClient.row(phone, "app_users_communities", membership["id"])["deleted_at"]
      [row] = for row <- SyncClient.rows(phone, "app_posts_communities"), row["post_id"] == post.id, do: row
      assert row["deleted_at"]
    end

    test "a device away longer than the retention window resyncs from nothing and ends up matching",
         %{owner: owner, other: other, community: community} do
      kept = post_fixture(owner, [community.id])
      gone = post_fixture(owner, [community.id])
      phone = SyncClient.pull(SyncClient.new(other))
      assert SyncClient.row(phone, "app_posts", gone.id)

      {:ok, _post} = Posting.delete(owner, gone)
      App.Data.Purge.run(now: DateTime.add(DateTime.utc_now(), 91, :day))
      assert Posts.get(gone.id, include_deleted: true) == nil

      phone = SyncClient.pull(phone)
      fresh = SyncClient.pull(SyncClient.new(other))

      assert [%{"resync" => true} | _] = phone.last_pull
      assert phone.rows == fresh.rows
      assert phone.cursor == fresh.cursor
      assert SyncClient.row(phone, "app_posts", kept.id)
      refute SyncClient.row(phone, "app_posts", gone.id)

      # A device in step with the server is not asked to resync, once the
      # horizon is where a real purge puts it, in the past
      App.Data.Purge.run()
      assert [%{"resync" => false}] = SyncClient.pull(phone).last_pull
    end
  end

  describe "push/2 authorization" do
    test "refuses a post with someone else's owner id, a membership for another user, and a community row from a non-owner",
         %{owner: owner, other: other, community: community} do
      theirs = post_fixture(other, [])
      now = DateTime.to_iso8601(DateTime.utc_now())
      membership = Memberships.membership(other, community.id)

      rows = [
        %{
          "table" => "app_posts",
          "row" => %{
            "id" => Ecto.UUID.generate(version: 7),
            "body" => "as them",
            "owner_id" => other.user.id,
            "updated_at" => now
          }
        },
        %{
          "table" => "app_users_communities",
          "row" => %{
            "id" => Ecto.UUID.generate(version: 7),
            "user_id" => other.user.id,
            "community_id" => community.id,
            "updated_at" => now
          }
        },
        %{
          "table" => "app_posts_communities",
          "row" => %{
            "id" => Ecto.UUID.generate(version: 7),
            "post_id" => theirs.id,
            "community_id" => community.id,
            "users_communities_id" => membership.id,
            "updated_at" => now
          }
        }
      ]

      assert %{"accepted" => [], "rejected" => rejected} = wire(Sync.push(owner, rows))
      # Applied parents first: the membership, then the post, then its community row
      assert Enum.map(rejected, & &1["reason"]) == ["not_own_membership", "events_only", "not_owner"]
      assert Posts.list(filters: [owner_id: other.user.id]) == [theirs]
      assert Posting.communities_of(other, theirs) == []
    end

    test "applies a post's event before its community row whatever the order sent, and a retried row gets the server's version",
         %{owner: owner, community: community} do
      now = DateTime.to_iso8601(DateTime.utc_now())
      post_id = Ecto.UUID.generate(version: 7)
      membership = Memberships.membership(owner, community.id)

      row = %{
        "table" => "app_posts_communities",
        "row" => %{
          "id" => Ecto.UUID.generate(version: 7),
          "post_id" => post_id,
          "community_id" => community.id,
          "users_communities_id" => membership.id,
          "updated_at" => now
        }
      }

      event = %{
        "id" => Ecto.UUID.generate(version: 7),
        "table" => "app_posts",
        "row_id" => post_id,
        "type" => "create",
        "base_version" => 0,
        "changes" => %{"body" => "child first"}
      }

      assert %{
               "events" => [%{"row_id" => ^post_id, "status" => "applied"}],
               "rejected" => [%{"table" => "app_posts_communities", "reason" => "server_wins"}]
             } = wire(Sync.push(owner, [row], [event]))

      assert [_row] = Posting.communities_of(owner, Posts.get(post_id))

      community_id = Ecto.UUID.generate(version: 7)

      community_row = %{
        "table" => "app_communities",
        "row" => %{"id" => community_id, "name" => "N", "updated_at" => now}
      }

      assert %{"accepted" => [%{"version" => 1}]} = wire(Sync.push(owner, [community_row]))

      assert %{"rejected" => [%{"reason" => "server_wins", "server" => %{"row" => %{"version" => 1}}}]} =
               wire(Sync.push(owner, [community_row]))
    end

    test "a community is created from a client but never changed",
         %{owner: owner, community: community} do
      now = DateTime.utc_now()
      future = now |> DateTime.add(365, :day) |> DateTime.to_iso8601()
      id = Ecto.UUID.generate(version: 7)

      assert %{"accepted" => [%{"table" => "app_communities"}]} =
               wire(
                 Sync.push(owner, [
                   %{
                     "table" => "app_communities",
                     "row" => %{"id" => id, "name" => "New", "updated_at" => DateTime.to_iso8601(now)}
                   }
                 ])
               )

      assert %{"rejected" => [%{"reason" => "server_wins", "server" => %{"row" => %{"name" => name}}}]} =
               wire(
                 Sync.push(owner, [
                   %{
                     "table" => "app_communities",
                     "row" => %{"id" => community.id, "name" => "Renamed", "updated_at" => future}
                   }
                 ])
               )

      assert name == community.name
    end

    test "a row that is not a map, and a cursor that is not a datetime, are refused without crashing", %{
      owner: owner
    } do
      assert %{"rejected" => [%{"id" => nil, "reason" => "malformed", "table" => nil}]} =
               wire(Sync.push(owner, ["nope"]))

      assert Sync.pull(owner, %{"cursor" => %{"last_synced_at" => "yesterday", "id" => "x"}}) ==
               {:error, :bad_cursor}

      assert Sync.pull(owner, %{"after" => %{"last_synced_at" => "2026", "id" => "x"}}) ==
               {:error, :bad_cursor}
    end
  end

  describe "pull/2 and tombstones" do
    test "a deleted post travels without its body, and a deleted or removed post reaches a member as its community row",
         %{owner: owner, other: other, community: community} do
      deleted = post_fixture(owner, [community.id], %{body: "secret, deleted"})
      removed = post_fixture(owner, [community.id], %{body: "secret, removed"})
      {:ok, _post} = Posting.delete(owner, deleted)
      {:ok, _row} = Posting.remove_from_community(owner, removed, community.id)

      laptop = SyncClient.pull(SyncClient.new(owner))
      assert SyncClient.row(laptop, "app_posts", deleted.id)["body"] == ""
      assert SyncClient.row(laptop, "app_posts", deleted.id)["deleted_at"]

      # A member's device prunes a post whose community row is gone
      phone = SyncClient.pull(SyncClient.new(other))
      refute SyncClient.row(phone, "app_posts", deleted.id)
      refute SyncClient.row(phone, "app_posts", removed.id)

      for post <- [deleted, removed] do
        [row] = for row <- SyncClient.rows(phone, "app_posts_communities"), row["post_id"] == post.id, do: row
        assert row["deleted_at"]
      end
    end

    test "a row written late inside the overlap window is still delivered, and a page holds each row once",
         %{owner: owner, other: other, community: community} do
      phone = SyncClient.pull(SyncClient.new(other))
      post = post_fixture(owner, [community.id])
      late = DateTime.add(DateTime.utc_now(), -(Sync.overlap_seconds() - 5), :second)

      App.Repo.update_all(from(p in App.Data.Post, where: p.id == ^post.id), set: [last_synced_at: late])

      phone = SyncClient.pull(phone)
      assert SyncClient.row(phone, "app_posts", post.id)

      for page <- phone.last_pull do
        keys = for %{"table" => table, "row" => row} <- page["rows"], do: {table, row["id"]}
        assert keys == Enum.uniq(keys)
      end
    end
  end

  describe "profiles" do
    defp profile_on(client), do: hd(SyncClient.rows(client, "app_user_profiles"))

    test "a user's own profile syncs, and only theirs", %{owner: owner} do
      phone = SyncClient.pull(SyncClient.new(owner))

      assert [%{"user_id" => user_id, "version" => 1}] = SyncClient.rows(phone, "app_user_profiles")
      assert user_id == owner.user.id
    end

    test "edits to different fields on two devices both apply, and the same field becomes a conflict",
         %{owner: owner} do
      phone = SyncClient.pull(SyncClient.new(owner))
      laptop = SyncClient.pull(SyncClient.new(owner))
      id = profile_on(phone)["id"]

      laptop =
        SyncClient.change(laptop, "app_user_profiles", id, %{"display_name" => "Ada", "bio" => "laptop"})

      {_laptop, %{"events" => [%{"status" => "applied", "version" => 2}]}} = SyncClient.push(laptop)

      phone = SyncClient.change(phone, "app_user_profiles", id, %{"bio" => "phone"})
      {phone, %{"events" => [answer]}} = SyncClient.push(phone)

      assert %{"status" => "conflict", "conflicts" => %{"bio" => %{"yours" => "phone", "server" => "laptop"}}} =
               answer

      assert %{"display_name" => "Ada", "bio" => "laptop"} = profile_on(phone)

      phone = SyncClient.change(phone, "app_user_profiles", id, %{"display_name" => "Ada L."})
      {phone, %{"events" => [%{"status" => "applied"}]}} = SyncClient.push(phone)

      [conflict] = phone.conflicts
      phone = SyncClient.resolve(phone, conflict["id"], :reapply)
      {phone, %{"events" => [%{"status" => "applied", "version" => 4}]}} = SyncClient.push(phone)

      assert %{"display_name" => "Ada L.", "bio" => "phone"} = profile_on(phone)
      assert %{display_name: "Ada L.", bio: "phone"} = Profiles.get(owner)
    end

    test "another user's profile, a profile as a row, a create and an unknown field are refused",
         %{owner: owner, other: other} do
      theirs = Profiles.get(other)

      event = fn changes, type ->
        %{
          "id" => Ecto.UUID.generate(version: 7),
          "table" => "app_user_profiles",
          "row_id" => theirs.id,
          "type" => type,
          "base_version" => 1,
          "changes" => changes
        }
      end

      %{"events" => answers} =
        wire(
          Sync.push(owner, [], [
            event.(%{"bio" => "mine now"}, "update"),
            event.(%{"bio" => "new"}, "create"),
            event.(%{"user_id" => owner.user.id}, "update")
          ])
        )

      assert Enum.map(answers, & &1["reason"]) == ["not_owner", "not_creatable", "not_editable: user_id"]

      row = %{
        "id" => theirs.id,
        "user_id" => owner.user.id,
        "updated_at" => DateTime.to_iso8601(DateTime.utc_now())
      }

      assert %{"rejected" => [%{"reason" => "events_only"}]} =
               wire(Sync.push(owner, [%{"table" => "app_user_profiles", "row" => row}]))

      assert Profiles.get(other).bio == nil
    end

    test "an invalid edit is refused with the changeset's errors", %{owner: owner} do
      phone = SyncClient.pull(SyncClient.new(owner))

      phone =
        SyncClient.change(phone, "app_user_profiles", profile_on(phone)["id"], %{
          "bio" => String.duplicate("x", 1001)
        })

      {_phone, %{"events" => [%{"status" => "rejected", "reason" => reason}]}} = SyncClient.push(phone)
      assert reason == "invalid: bio should be at most %{count} character(s)"
    end
  end

  describe "backfill/3" do
    test "returns a community's existing posts, paged", %{owner: owner, other: other} do
      community = joined_community_fixture(other)
      posts = for _ <- 1..3, do: post_fixture(other, [community.id])
      phone = SyncClient.pull(SyncClient.new(owner))
      refute SyncClient.rows(phone, "app_posts") |> Enum.any?(&(&1["owner_id"] == other.user.id))

      {:ok, _membership} = Memberships.join(owner, community.id)
      phone = SyncClient.backfill(phone, community.id, limit: 2)

      assert length(phone.last_pull) == 2

      assert Enum.sort(Enum.map(SyncClient.rows(phone, "app_posts"), & &1["id"])) ==
               Enum.sort(Enum.map(posts, & &1.id))

      assert length(SyncClient.rows(phone, "app_posts_communities")) == 3
    end

    test "refuses a non-member", %{owner: owner, other: other} do
      community = joined_community_fixture(other)
      assert Sync.backfill(owner, community.id, %{}) == {:error, :not_a_member}
    end
  end
end
