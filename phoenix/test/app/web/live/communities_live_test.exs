defmodule App.Web.CommunitiesLiveTest do
  use App.Web.ConnCase

  import Phoenix.LiveViewTest
  import App.Data.CommunitiesFixtures
  import App.Data.UsersFixtures

  alias App.Data.Memberships

  setup %{conn: conn} do
    user = user_fixture()
    scope = user_scope_fixture(user)
    %{conn: conn |> log_in_user(user), scope: scope}
  end

  test "lists every community", %{conn: conn, scope: scope} do
    community = community_fixture()
    joined = joined_community_fixture(scope)

    {:ok, view, _html} = live(conn, ~p"/communities")

    assert has_element?(view, "#communities-#{community.id} button", "Join")
    assert has_element?(view, "#communities-#{joined.id} button", "Leave")
    assert has_element?(view, "#communities-#{joined.id} a", joined.name)
  end

  test "creates a community and joins it", %{conn: conn, scope: scope} do
    {:ok, view, _html} = live(conn, ~p"/communities")

    view
    |> form("#community-form", community: %{name: "Gardeners"})
    |> render_submit()

    assert has_element?(view, "#communities a", "Gardeners")
    assert [community] = App.Data.Communities.list()
    assert Memberships.member?(scope, community.id)
  end

  test "requires a name", %{conn: conn} do
    {:ok, view, _html} = live(conn, ~p"/communities")

    html =
      view
      |> form("#community-form", community: %{name: ""})
      |> render_submit()

    assert html =~ "can&#39;t be blank"
  end

  test "joins and leaves", %{conn: conn, scope: scope} do
    community = community_fixture()
    {:ok, view, _html} = live(conn, ~p"/communities")

    view |> element("#communities-#{community.id} button", "Join") |> render_click()
    assert Memberships.member?(scope, community.id)
    assert has_element?(view, "#communities-#{community.id} button", "Leave")

    view |> element("#communities-#{community.id} button", "Leave") |> render_click()
    refute Memberships.member?(scope, community.id)
    assert has_element?(view, "#communities-#{community.id} button", "Join")
  end

  test "redirects to the login page without a user" do
    assert {:error, {:redirect, %{to: path}}} = live(build_conn(), ~p"/communities")
    assert path == ~p"/users/log-in"
  end
end
