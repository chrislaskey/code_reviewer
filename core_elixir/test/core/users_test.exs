defmodule Core.UsersTest do
  use Core.BackendCase, async: false

  alias Core.App.Users
  alias Core.Helpers.{Ids, Time}

  defp rename_event(user, base_version, base_name, name) do
    %{
      "id" => Ids.new(),
      "table" => "app_users",
      "row_id" => user["id"],
      "type" => "update",
      "base_version" => base_version,
      "base" => %{"name" => base_name},
      "changes" => %{"name" => name},
      "made_at" => Time.to_wire(Time.now())
    }
  end

  test "a person renames themselves, and the same name writes nothing" do
    alice = signed_up("alice@example.com")["user"]
    assert {:ok, %{"name" => "Alice", "version" => 2}} = Users.update(alice["id"], "  Alice ")
    assert {:ok, %{"name" => "Alice", "version" => 2}} = Users.update(alice["id"], "Alice")
    assert {:error, {:invalid, %{"name" => _}}} = Users.update(alice["id"], " ")
  end

  test "a pushed rename from an old version is a conflict, and the server's name stays" do
    alice = signed_up("alice@example.com")["user"]
    {:ok, _} = Users.update(alice["id"], "From the web")
    event = rename_event(alice, alice["version"], alice["name"], "From the device")
    {:ok, %{"events" => [answer]}} = Core.Sync.push(alice["id"], [event], [])
    assert %{"status" => "conflict", "conflicts" => %{"name" => %{"server" => "From the web"}}} = answer
  end

  test "nobody renames someone else, or makes a user" do
    alice = signed_up("alice@example.com")["user"]
    bob = signed_up("bob@example.com")["user"]
    event = rename_event(alice, alice["version"], alice["name"], "Not Alice")
    {:ok, %{"events" => [answer]}} = Core.Sync.push(bob["id"], [event], [])
    assert %{"status" => "rejected", "reason" => "not_yours"} = answer
    create = %{event | "id" => Ids.new(), "type" => "create"}
    {:ok, %{"events" => [answer]}} = Core.Sync.push(alice["id"], [create], [])
    assert %{"status" => "rejected", "reason" => "server_only"} = answer
  end

  test "a picture is set as an event, and listed for the sweep, a deleted user's too" do
    alice = signed_up("alice@example.com")["user"]
    url = "https://bucket.example.com/avatars/#{alice["id"]}/one.png"
    assert {:ok, %{"avatar_url" => ^url, "version" => 2}} = Users.avatar_update(alice["id"], url)
    assert Users.avatar_urls_list() == {:ok, %{"avatar_urls" => [url]}}
    assert Users.avatar_update(Ids.new(), url) == {:error, :not_found}
  end
end
