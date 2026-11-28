defmodule Core.PasswordTest do
  use Core.BackendCase, async: false

  alias Core.Accounts
  alias Core.Accounts.{Password, TrialUsers}
  alias Core.App.Users

  @hash "$argon2id$v=19$m=19456,t=2,p=1$c29tZXNhbHQ$aGFzaA"

  test "a password is kept as its hash, and the change mailed to the account" do
    alice = signed_up("alice@example.com")["user"]["id"]
    assert Password.get(alice) == {:ok, %{"is_set" => false}}
    assert Password.get_hash("alice@example.com") == {:ok, %{"user_id" => alice, "password_hash" => nil}}

    assert {:ok, %{"mail" => %{"kind" => "password_changed", "to" => "alice@example.com"}}} =
             Password.update(alice, @hash, "en")

    assert Password.get(alice) == {:ok, %{"is_set" => true}}
    assert Password.get_hash(" Alice@Example.com ") == {:ok, %{"user_id" => alice, "password_hash" => @hash}}
    {:ok, _} = Password.update(alice, "another", "en")
    assert {:ok, %{"password_hash" => "another"}} = Password.get_hash("alice@example.com")
  end

  test "only a confirmed account, no trial, has a hash to check" do
    {:ok, _} = Accounts.sign_in("carol@example.com", true, "en")
    {:ok, guest} = TrialUsers.create(10)
    assert Password.get_hash("carol@example.com") == {:ok, %{"user_id" => nil, "password_hash" => nil}}
    assert Password.get_hash(guest["email"]) == {:ok, %{"user_id" => nil, "password_hash" => nil}}
    assert Password.get_hash("nobody@example.com") == {:ok, %{"user_id" => nil, "password_hash" => nil}}
    assert Accounts.sign_in_with_password(guest["user"]["id"], "2026-11-22") == {:error, :sign_in_expired}
  end

  test "a sign-in Rust proved with the password ends in a token, or a device's code" do
    alice = signed_up("alice@example.com")["user"]["id"]
    {:ok, session} = Accounts.sign_in_with_password(alice, "2026-11-22")
    assert {:ok, %{"user" => %{"id" => ^alice}}} = Users.get_by_token(session["token"])
    {:ok, %{"code" => code}} = Accounts.sign_in_with_password_for_device(alice, "a challenge", "2026-11-22")
    assert {:ok, %{"user" => %{"id" => ^alice}}} = Accounts.sign_in_with_device_code(code, "a challenge")
  end
end
