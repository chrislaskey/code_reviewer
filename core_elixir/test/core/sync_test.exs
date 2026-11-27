defmodule Core.SyncTest do
  use Core.BackendCase, async: false

  alias Core.Sync
  alias Core.App.{Files, Memberships, Users}
  alias Core.Helpers.Ids

  defp push_file(user_id, community_id, name) do
    file_id = Ids.new()

    {:ok, %{"events" => [%{"status" => "applied"}]}} =
      Sync.push(user_id, [file_create(file_id, community_id, name)], [])

    file_id
  end

  test "sync_a_sign_up_pulls_its_user_community_and_membership_parents_first" do
    %{"user" => %{"id" => alice}} = signed_up("alice@example.com")
    {rows, cursor} = pull_all(alice, nil)

    assert Enum.map(rows, & &1["table"]) == ["app_users", "app_communities", "app_users_communities"]
    assert %{"global_sync_version" => version} = cursor
    assert is_binary(version)
    assert Enum.all?(rows, &is_binary(&1["row"]["global_sync_version"]))
  end

  test "sync_a_token_resolves_to_its_user_and_kind" do
    %{"user" => %{"id" => alice}, "token" => token} = signed_up("alice@example.com")
    assert {:ok, %{"user" => %{"id" => ^alice}, "kind" => "account"}} = Users.get_by_token(token)
    assert {:error, :unknown_token} = Users.get_by_token("nope")
  end

  test "sync_a_pushed_file_create_is_applied_published_and_answered_again_on_retry" do
    %{"user" => %{"id" => alice}} = signed_up("alice@example.com")
    community = personal_community(alice)
    {_rows, cursor} = pull_all(alice, nil)
    file_id = Ids.new()
    event = file_create(file_id, community, "notes.txt")

    {:ok, %{"events" => [answer]}} = Sync.push(alice, [event], [])
    assert %{"status" => "applied", "version" => 1, "server" => %{"table" => "app_files", "row" => row}} = answer
    assert row["name"] == "notes.txt"
    assert is_binary(row["global_sync_version"])

    {:ok, %{"events" => [again]}} = Sync.push(alice, [event], [])
    assert again["status"] == "applied"
    assert again["server"]["row"]["global_sync_version"] == row["global_sync_version"]

    {rows, _cursor} = pull_all(alice, cursor)
    assert ids_of(rows, "app_files") == [file_id]
    assert length(ids_of(rows, "app_files_communities")) == 1
  end

  test "sync_a_create_made_on_a_whole_second_is_applied" do
    %{"user" => %{"id" => alice}} = signed_up("alice@example.com")
    file_id = Ids.new()
    # Rust writes a time on a whole second with no fraction
    event = %{file_create(file_id, personal_community(alice), "notes.txt") | "made_at" => "2026-11-27T10:00:00Z"}

    {:ok, %{"events" => [answer]}} = Sync.push(alice, [event], [])
    assert %{"status" => "applied", "server" => %{"row" => row}} = answer
    assert row["inserted_at"] == "2026-11-27T10:00:00.000000Z"
  end

  test "sync_a_file_create_without_its_bytes_or_community_is_rejected" do
    %{"user" => %{"id" => alice}} = signed_up("alice@example.com")
    %{"user" => %{"id" => bob}} = signed_up("bob@example.com")
    community = personal_community(alice)

    no_bytes = update_in(file_create(Ids.new(), community, "a.txt"), ["changes"], &Map.delete(&1, "content"))

    assert {:ok, %{"events" => [%{"status" => "rejected", "reason" => "file_not_uploaded"}]}} =
             Sync.push(alice, [no_bytes], [])

    assert {:ok, %{"events" => [%{"status" => "applied"}]}} = Sync.push(alice, [no_bytes], [no_bytes["id"]])

    not_hers = file_create(Ids.new(), community, "b.txt")

    assert {:ok, %{"events" => [%{"status" => "rejected", "reason" => "not_a_member"}]}} =
             Sync.push(bob, [not_hers], [])
  end

  test "sync_two_renames_from_the_same_version_keep_the_first_and_answer_a_conflict" do
    %{"user" => %{"id" => alice}} = signed_up("alice@example.com")
    file_id = push_file(alice, personal_community(alice), "a.txt")

    rename = fn name ->
      %{
        "id" => Ids.new(),
        "table" => "app_files",
        "row_id" => file_id,
        "type" => "update",
        "base_version" => 1,
        "base" => %{"name" => "a.txt"},
        "changes" => %{"name" => name},
        "made_at" => nil
      }
    end

    {:ok, %{"events" => [first]}} = Sync.push(alice, [rename.("b.txt")], [])
    assert %{"status" => "applied", "version" => 2} = first

    {:ok, %{"events" => [second]}} = Sync.push(alice, [rename.("c.txt")], [])
    assert second["status"] == "conflict"
    assert second["conflicts"] == %{"name" => %{"yours" => "c.txt", "server" => "b.txt"}}
    assert second["server"]["row"]["name"] == "b.txt"
  end

  test "sync_taking_a_file_out_of_its_last_community_deletes_it" do
    %{"user" => %{"id" => alice}} = signed_up("alice@example.com")
    file_id = push_file(alice, personal_community(alice), "a.txt")
    {_rows, cursor} = pull_all(alice, nil)

    assert :ok = Files.delete(alice, file_id)
    assert {:error, :not_found} = Files.get(alice, file_id)

    {rows, _cursor} = pull_all(alice, cursor)
    [link] = for %{"table" => "app_files_communities", "row" => row} <- rows, do: row
    assert link["deleted_at"]
  end

  test "sync_pull_pages_by_visible_rows_and_hides_other_users_files" do
    %{"user" => %{"id" => alice}} = signed_up("alice@example.com")
    %{"user" => %{"id" => bob}} = signed_up("bob@example.com")
    {_rows, cursor} = pull_all(alice, nil)

    for index <- 1..3, do: push_file(bob, personal_community(bob), "bob #{index}")
    mine = for index <- 1..3, do: push_file(alice, personal_community(alice), "alice #{index}")

    {:ok, page} = Sync.pull(alice, %{"cursor" => cursor, "limit" => 2})
    assert page["next"]
    assert Enum.all?(ids_of(page["rows"], "app_files"), &(&1 in mine))

    {rows, _cursor} = pull_all(alice, cursor, 2)
    assert Enum.sort(ids_of(rows, "app_files")) == Enum.sort(mine)
  end

  test "sync_a_cursor_from_another_feed_or_above_the_head_resyncs" do
    %{"user" => %{"id" => alice}} = signed_up("alice@example.com")
    foreign = %{"feed_id" => Ids.new(), "global_sync_version" => "1"}
    assert {:ok, %{"resync" => true, "rows" => []}} = Sync.pull(alice, %{"cursor" => foreign})

    {_rows, cursor} = pull_all(alice, nil)
    above = %{cursor | "global_sync_version" => "999"}
    assert {:ok, %{"resync" => true}} = Sync.pull(alice, %{"cursor" => above})
  end

  test "sync_joining_a_community_backfills_its_files_and_leaving_is_published" do
    %{"user" => %{"id" => alice}} = signed_up("alice@example.com")
    %{"user" => %{"id" => bob}} = signed_up("bob@example.com")
    gardening = personal_community(alice)
    file_id = push_file(alice, gardening, "roses.txt")

    assert {:error, :not_a_member} = Sync.backfill(bob, %{"community_id" => gardening})
    {:ok, _membership} = Memberships.create(bob, gardening)
    {:ok, page} = Sync.backfill(bob, %{"community_id" => gardening})
    assert ids_of(page["rows"], "app_files") == [file_id]
    assert page["cursor"] == nil

    {_rows, cursor} = pull_all(bob, nil)
    {:ok, %{}} = Memberships.delete(bob, gardening)
    {rows, _cursor} = pull_all(bob, cursor)
    [membership] = for %{"table" => "app_users_communities", "row" => row} <- rows, do: row
    assert membership["deleted_at"]
  end
end
