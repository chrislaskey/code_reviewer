defmodule Core.PreferencesTest do
  use Core.BackendCase, async: false

  alias Core.Accounts
  alias Core.Accounts.Preferences
  alias Core.Helpers.{Ids, Time}

  defp create_event(user_id, key, value) do
    %{
      "id" => Ids.new(),
      "table" => "accounts_preferences",
      "row_id" => Ids.new(),
      "type" => "create",
      "base_version" => 0,
      "base" => %{},
      "changes" => %{"user_id" => user_id, "key" => key, "value" => value},
      "made_at" => Time.to_wire(Time.now())
    }
  end

  test "a preference is set, read, and set again" do
    alice = signed_up("alice@example.com")["user"]["id"]
    assert Preferences.get(alice) == %{"locale" => nil, "theme" => nil}
    assert {:ok, %{"locale" => "es", "theme" => nil}} = Preferences.update(alice, "locale", "es")
    assert {:ok, %{"locale" => nil}} = Preferences.update(alice, "locale", nil)
    assert {:ok, %{"theme" => "dark"}} = Preferences.update(alice, "theme", "dark")

    assert Preferences.update(alice, "theme", "plaid") ==
             {:error, {:refused, ~s(invalid: not a value for theme: "plaid")}}

    assert Preferences.update(alice, "colour", "red") == {:error, {:refused, "invalid: unknown preference: colour"}}
  end

  test "a pushed create of a key the user has writes over that row, and a retry is answered applied" do
    alice = signed_up("alice@example.com")["user"]["id"]
    {:ok, _} = Preferences.update(alice, "locale", "es")
    event = create_event(alice, "locale", "en")
    {:ok, %{"events" => [answer]}} = Core.Sync.push(alice, [event], [])
    assert %{"status" => "applied", "version" => 2, "server" => %{"row" => %{"value" => "en"} = row}} = answer
    assert row["id"] != event["row_id"]
    # As in Rust, a retry finds the event's row id, the device's, which the server's row replaced
    assert {:ok, %{"events" => [%{"status" => "applied", "version" => 2}]}} = Core.Sync.push(alice, [event], [])
    assert Preferences.get(alice)["locale"] == "en"
  end

  test "a pushed event of another user's preference is refused" do
    alice = signed_up("alice@example.com")["user"]["id"]
    bob = signed_up("bob@example.com")["user"]["id"]
    {:ok, %{"events" => [answer]}} = Core.Sync.push(bob, [create_event(alice, "theme", "dark")], [])
    assert %{"status" => "rejected", "reason" => "not_yours"} = answer
  end

  test "a pull brings the user's own preferences only" do
    alice = signed_up("alice@example.com")["user"]["id"]
    bob = signed_up("bob@example.com")["user"]["id"]
    {:ok, _} = Preferences.update(alice, "theme", "dark")
    {alice_rows, _} = pull_all(alice, nil)
    {bob_rows, _} = pull_all(bob, nil)
    assert length(ids_of(alice_rows, "accounts_preferences")) == 1
    assert ids_of(bob_rows, "accounts_preferences") == []
  end

  test "a code is mailed in the language the account chose" do
    alice = signed_up("alice@example.com")["user"]["id"]
    {:ok, _} = Preferences.update(alice, "locale", "es")
    assert {:ok, %{"mail" => %{"locale" => "es"}}} = Accounts.sign_in("alice@example.com", true, "en")
    assert {:ok, %{"mail" => %{"locale" => "en"}}} = Accounts.sign_in("bob@example.com", true, "zz")
  end
end
