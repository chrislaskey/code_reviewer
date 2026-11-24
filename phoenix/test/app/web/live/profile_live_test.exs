defmodule App.Web.ProfileLiveTest do
  use App.Web.ConnCase

  import Phoenix.LiveViewTest
  import App.Data.UsersFixtures

  alias App.Data.Profiles

  setup %{conn: conn} do
    user = user_fixture()
    %{conn: log_in_user(conn, user), scope: user_scope_fixture(user)}
  end

  defp save(view, params), do: view |> form("#profile-form", profile: params) |> render_submit()

  test "saves the profile", %{conn: conn, scope: scope} do
    {:ok, view, _html} = live(conn, ~p"/profile")

    assert save(view, %{display_name: "Ada", bio: "Hello"}) =~ "Profile saved."
    assert %{display_name: "Ada", bio: "Hello", version: 2} = Profiles.get(scope)
  end

  test "a field changed elsewhere since the form loaded is kept, listed, and can be reapplied or let go",
       %{conn: conn, scope: scope} do
    {:ok, view, _html} = live(conn, ~p"/profile")
    {:ok, _answer, _profile} = Profiles.save(scope, Profiles.get(scope), %{"display_name" => "Elsewhere"})

    save(view, %{display_name: "Here", bio: "Mine"})

    assert has_element?(view, "#conflict-display_name", "Your change: Here")
    assert has_element?(view, "#conflict-display_name", "Now: Elsewhere")
    assert %{display_name: "Elsewhere", bio: "Mine"} = Profiles.get(scope)

    view |> element("#conflict-display_name button", "Reapply") |> render_click()
    refute has_element?(view, "#profile-conflicts")
    assert Profiles.get(scope).display_name == "Here"

    {:ok, _answer, _profile} = Profiles.save(scope, Profiles.get(scope), %{"bio" => "Elsewhere"})
    save(view, %{display_name: "Here", bio: "Again"})
    view |> element("#conflict-bio button", "Let it go") |> render_click()
    refute has_element?(view, "#profile-conflicts")
    assert Profiles.get(scope).bio == "Elsewhere"
  end

  test "shows the changeset's errors", %{conn: conn} do
    {:ok, view, _html} = live(conn, ~p"/profile")
    assert save(view, %{display_name: String.duplicate("x", 101)}) =~ "should be at most 100 character"
  end
end
