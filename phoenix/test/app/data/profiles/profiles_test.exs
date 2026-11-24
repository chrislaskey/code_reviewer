defmodule App.Data.ProfilesTest do
  use App.DataCase

  import App.Data.UsersFixtures

  alias App.Data.Accounts
  alias App.Data.Profiles
  alias App.Data.UserProfiles

  setup do
    %{scope: user_scope_fixture(user_fixture())}
  end

  test "registering creates an empty profile, and deleting the account deletes it", %{scope: scope} do
    profile = Profiles.get(scope)
    assert %{display_name: nil, bio: nil, version: 1} = profile

    {:ok, _user} = Accounts.delete_user(scope.user)
    assert UserProfiles.get(profile.id) == nil
    assert UserProfiles.get(profile.id, include_deleted: true).deleted_by_id == scope.user.id
  end

  test "a save sends only the fields that changed from the form as loaded", %{scope: scope} do
    loaded = Profiles.get(scope)
    {:ok, _answer, _profile} = Profiles.save(scope, loaded, %{"display_name" => "Ada", "bio" => ""})

    # Saved again from the same stale form: the bio was not changed, so
    # only the display name is compared, and it conflicts
    {:ok, %{conflicts: conflicts}, profile} =
      Profiles.save(scope, loaded, %{"display_name" => "Grace", "bio" => ""})

    assert conflicts == %{"display_name" => %{"yours" => "Grace", "server" => "Ada"}}
    assert %{display_name: "Ada", bio: nil, version: 2} = profile
  end

  test "an invalid save returns the changeset", %{scope: scope} do
    assert {:error, %Ecto.Changeset{} = changeset} =
             Profiles.save(scope, Profiles.get(scope), %{"display_name" => String.duplicate("x", 101)})

    assert changeset.errors[:display_name]
  end
end
