defmodule Core.DeletionsTest do
  use Core.BackendCase, async: false

  alias Core.Accounts.{Deletions, Preferences, TrialUsers}
  alias Core.App.{Communities, Memberships, Users}
  alias Core.Data.{AccountsUsers, AppCommunities, AppFiles, AppFilesCommunities, AppUsers, AppUsersCommunities}
  alias Core.Helpers.Ids

  defp code_for(user_id) do
    {:ok, %{"mail" => %{"kind" => "delete_code", "code" => code}}} = Deletions.code_create(user_id, "en")
    code
  end

  test "a full user's delete takes their community and files, leaves the shared one, and keeps their row as deleted" do
    alice = signed_up("alice@example.com")
    bob = signed_up("bob@example.com")
    alice_id = alice["user"]["id"]
    personal = personal_community(alice_id)
    shared = personal_community(bob["user"]["id"])
    {:ok, _} = Memberships.create(alice_id, shared)
    {:ok, _} = Preferences.update(alice_id, "theme", "dark")
    file_id = Ids.new()

    {:ok, %{"events" => [%{"status" => "applied"}]}} =
      Core.Sync.push(alice_id, [file_create(file_id, personal, "a.txt")], [])

    code = code_for(alice_id)
    assert Deletions.create(alice_id, "000000" <> "x", "en") == {:error, :wrong_code}

    assert {:ok, %{"mail" => %{"kind" => "account_deleted", "to" => "alice@example.com"}}} =
             Deletions.create(alice_id, code, "en")

    assert Users.get_by_token(alice["token"]) == {:error, :account_deleted}
    assert %AppUsers{name: "Deleted user", deleted_at: %DateTime{}} = AppUsers.get(alice_id)
    assert %AccountsUsers{deleted_at: %DateTime{}} = AccountsUsers.get(alice_id)
    assert %AppCommunities{deleted_at: %DateTime{}} = AppCommunities.get(personal)
    assert %AppCommunities{deleted_at: nil} = AppCommunities.get(shared)

    assert %AppUsersCommunities{deleted_at: %DateTime{}} =
             AppUsersCommunities.get_by_user_and_community(alice_id, shared)

    assert %AppFiles{deleted_at: %DateTime{}} = AppFiles.get(file_id)
    assert AppFilesCommunities.list_live_of_community(personal) == []
    assert Preferences.get(alice_id) == %{"locale" => nil, "theme" => nil}
    assert "Personal" in for(%{"community" => %{"name" => n}} <- Communities.list(bob["user"]["id"]), do: n)
  end

  test "a code is needed, and the fifth wrong one ends it" do
    alice = signed_up("alice@example.com")["user"]["id"]
    assert Deletions.create(alice, "123456", "en") == {:error, :wrong_code}
    code = code_for(alice)
    for _ <- 1..5, do: assert(Deletions.create(alice, "wrong", "en") == {:error, :wrong_code})
    assert Deletions.create(alice, code, "en") == {:error, :wrong_code}
    assert %AccountsUsers{deleted_at: nil} = AccountsUsers.get(alice)
  end

  test "a trial goes outright, with no code" do
    {:ok, guest} = TrialUsers.create(10)
    id = guest["user"]["id"]
    assert Deletions.code_create(id, "en") == {:error, :trial_has_no_code}
    assert Deletions.create(id, nil, "en") == {:ok, %{"mail" => nil}}
    assert AccountsUsers.get(id) == nil and AppUsers.get(id) == nil
    assert Deletions.create(id, nil, "en") == {:error, :not_found}
  end
end
