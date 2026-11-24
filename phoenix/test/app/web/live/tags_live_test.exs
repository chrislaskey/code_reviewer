defmodule App.Web.TagsLiveTest do
  use App.Web.ConnCase

  import Phoenix.LiveViewTest
  import App.Data.PostsFixtures
  import App.Data.UsersFixtures

  alias App.Data.Tagging

  setup %{conn: conn} do
    user = user_fixture()
    scope = user_scope_fixture(user)
    %{conn: conn |> log_in_user(user), scope: scope}
  end

  test "makes a tag, and refuses a second of the same name at once", %{conn: conn, scope: scope} do
    {:ok, view, _html} = live(conn, ~p"/tags")

    view |> form("#new-tag-form", %{name: "Elixir"}) |> render_submit()
    assert [{%{name: "Elixir"} = tag, 0}] = Tagging.tags_with_counts(scope)
    assert has_element?(view, "#tag-#{tag.id}", "0 posts")

    html = view |> form("#new-tag-form", %{name: "elixir"}) |> render_submit()
    assert html =~ "Name has already been taken"
    assert length(Tagging.tags_with_counts(scope)) == 1
  end

  test "renames a tag, and deleting it takes it off its posts and leaves them", %{conn: conn, scope: scope} do
    post = post_fixture(scope, [], %{body: "Mine"})
    {:ok, _row} = Tagging.tag_post(scope, post, "elixir")
    [{tag, 1}] = Tagging.tags_with_counts(scope)

    {:ok, view, _html} = live(conn, ~p"/tags")
    assert has_element?(view, "#tag-#{tag.id}", "1 post")

    view |> form("#rename-tag-#{tag.id}", %{name: "Erlang"}) |> render_submit()
    assert has_element?(view, "#tag-#{tag.id}", "Erlang")

    view |> element("#tag-#{tag.id} button", "Delete") |> render_click()

    refute has_element?(view, "#tag-#{tag.id}")
    assert Tagging.tags_of(post.id) == []
    assert App.Data.Posts.get(post.id)
  end
end
