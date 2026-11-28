defmodule Core.TrialUsersTest do
  use Core.BackendCase, async: false

  alias Core.Accounts
  alias Core.Accounts.TrialUsers
  alias Core.App.{Communities, Memberships, Users}
  alias Core.Data.{AccountsUsers, AppEvents, AppUsers}
  alias Core.Helpers.{Ids, Time}
  alias Core.Repo

  defp trial do
    {:ok, session} = TrialUsers.create(10)
    session
  end

  defp community_names(user_id), do: for(%{"community" => %{"name" => name}} <- Communities.list(user_id), do: name)

  test "a trial has its own community, which nobody else sees or joins" do
    guest = trial()
    alice = signed_up("alice@example.com")
    assert %{"kind" => "trial"} = elem(Users.get_by_token(guest["token"]), 1)
    assert community_names(guest["user"]["id"]) == ["Guest"]
    assert community_names(alice["user"]["id"]) == ["Personal"]

    guest_community = personal_community(guest["user"]["id"])
    assert Memberships.create(alice["user"]["id"], guest_community) == {:error, :not_found}
    assert Memberships.create(guest["user"]["id"], personal_community(alice["user"]["id"])) == {:error, :not_found}
  end

  test "trials stop at the cap" do
    {:ok, _} = TrialUsers.create(1)
    assert TrialUsers.create(1) == {:error, :full}
    assert TrialUsers.count() == {:ok, %{"count" => 1}}
  end

  test "a trial's writes stop at the limit, and the refused one writes nothing" do
    guest = trial()
    user_id = guest["user"]["id"]
    community = personal_community(user_id)
    made = Repo.aggregate(AppEvents, :count)
    assert made == 2

    for _ <- 1..div(TrialUsers.max_writes() - made, 2) do
      {:ok, _} = Memberships.delete(user_id, community)
      {:ok, _} = Memberships.create(user_id, community)
    end

    assert Memberships.delete(user_id, community) == {:error, :trial_writes_full}
    assert Repo.aggregate(AppEvents, :count) == TrialUsers.max_writes()
    assert personal_community(user_id) == community
  end

  test "signing up from a trial keeps the user and shows their community" do
    guest = trial()
    user_id = guest["user"]["id"]

    {:ok, %{"pending" => pending, "mail" => %{"to" => "bob@example.com", "code" => code}}} =
      Accounts.sign_in_from_trial(user_id, "bob@example.com", "en")

    assert AppUsers.get(user_id).name == "bob"
    assert {:ok, %{"email" => "bob@example.com"}} = Accounts.resend_code_to(pending)
    {:ok, %{"pending" => confirm, "new_account" => true}} = Accounts.sign_in_with_code(pending, code)
    {:ok, session} = Accounts.sign_in_confirm(confirm, "2026-11-22")

    assert session["user"]["id"] == user_id
    assert Users.get_by_token(guest["token"]) == {:error, :unknown_token}
    assert {:ok, %{"kind" => "account", "email" => "bob@example.com"}} = Users.get_by_token(session["token"])
    account = AccountsUsers.get(user_id)
    assert account.trial_expires_at == nil and account.confirmed_at != nil

    alice = signed_up("alice@example.com")
    assert "Guest" in community_names(alice["user"]["id"])
  end

  test "a trial signing up with a confirmed account's address gets that account's code" do
    alice = signed_up("alice@example.com")
    guest = trial()

    {:ok, %{"pending" => pending, "mail" => %{"code" => code}}} =
      Accounts.sign_in_from_trial(guest["user"]["id"], "alice@example.com", "en")

    {:ok, %{"pending" => confirm, "new_account" => false}} = Accounts.sign_in_with_code(pending, code)
    {:ok, session} = Accounts.sign_in_confirm(confirm, "2026-11-22")
    assert session["user"]["id"] == alice["user"]["id"]
    assert {:ok, %{"kind" => "trial"}} = Users.get_by_token(guest["token"])
  end

  test "only a trial signs up this way" do
    alice = signed_up("alice@example.com")
    assert Accounts.sign_in_from_trial(alice["user"]["id"], "bob@example.com", "en") == {:error, :not_a_trial}
    assert Accounts.sign_in_from_trial(Ids.new(), "bob@example.com", "en") == {:error, :wrong_code}
  end

  test "expired trials go outright, a day after they end; a signed-up one stays" do
    expired = trial()
    signed_up = trial()

    {:ok, %{"pending" => pending, "mail" => %{"code" => code}}} =
      Accounts.sign_in_from_trial(signed_up["user"]["id"], "bob@example.com", "en")

    {:ok, %{"pending" => confirm}} = Accounts.sign_in_with_code(pending, code)
    {:ok, _} = Accounts.sign_in_confirm(confirm, "2026-11-22")

    assert TrialUsers.delete_expired(DateTime.add(Time.now(), 47, :hour)) == {:ok, %{"users" => 0}}
    assert TrialUsers.delete_expired(DateTime.add(Time.now(), 49, :hour)) == {:ok, %{"users" => 1}}
    assert AccountsUsers.get(expired["user"]["id"]) == nil
    assert AppUsers.get(expired["user"]["id"]) == nil
    assert AppUsers.get(signed_up["user"]["id"]) != nil
    assert Repo.aggregate(AppEvents, :count) > 0
  end
end
