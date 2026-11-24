defmodule App.Web.PostsLiveTest do
  use App.Web.ConnCase

  import Phoenix.LiveViewTest
  import App.Data.CommunitiesFixtures
  import App.Data.PostsFixtures
  import App.Data.UsersFixtures

  alias App.Data.Links
  alias App.Data.Posting
  alias App.Data.Tagging
  alias App.Web.Scope

  setup %{conn: conn} do
    user = user_fixture()
    scope = user_scope_fixture(user)
    %{conn: conn |> log_in_user(user), scope: scope}
  end

  test "writes a post into the ticked communities", %{conn: conn, scope: scope} do
    community = joined_community_fixture(scope)
    {:ok, view, _html} = live(conn, ~p"/posts")

    view
    |> form("#post-form form", post: %{body: "Hello", community_ids: [community.id]})
    |> render_submit()

    assert has_element?(view, "#posts li", "Hello")
    assert [post] = Posting.feed(Scope.put_community(scope, community))
    assert post.body == "Hello"
    refute view |> element("#post-form textarea") |> render() =~ "Hello"
  end

  test "lists the user's communities as choices", %{conn: conn, scope: scope} do
    community = joined_community_fixture(scope)
    {:ok, view, _html} = live(conn, ~p"/posts")

    assert has_element?(view, ~s(#post-form input[type="checkbox"][value="#{community.id}"]))
  end

  test "requires a body", %{conn: conn, scope: scope} do
    {:ok, view, _html} = live(conn, ~p"/posts")

    html =
      view
      |> form("#post-form form", post: %{body: ""})
      |> render_submit()

    assert html =~ "can&#39;t be blank"
    assert Posting.feed(scope) == []
  end

  test "limits a body to 10,000 characters", %{conn: conn} do
    {:ok, view, _html} = live(conn, ~p"/posts")

    html =
      view
      |> form("#post-form form", post: %{body: String.duplicate("x", 10_001)})
      |> render_submit()

    assert html =~ "should be at most 10000 character(s)"
  end

  test "lists what the user can read and deletes their own", %{conn: conn, scope: scope} do
    community = joined_community_fixture(scope)
    post = post_fixture(scope, [community.id], %{body: "Mine"})

    other = user_scope_fixture(user_fixture())
    hidden = post_fixture(other, [], %{body: "Theirs alone"})

    {:ok, view, _html} = live(conn, ~p"/posts")

    assert has_element?(view, "#posts-#{post.id}", "Mine")
    refute has_element?(view, "#posts-#{hidden.id}")

    view |> element("#posts-#{post.id} button", "Delete") |> render_click()

    refute has_element?(view, "#posts-#{post.id}")
    assert Posting.feed(scope) == []
  end

  test "the owner adds and removes a link, and a bad one shows its error at once", %{conn: conn, scope: scope} do
    post = post_fixture(scope, [], %{body: "Mine"})
    {:ok, view, _html} = live(conn, ~p"/posts")

    html =
      view
      |> form("#link-form-#{post.id}", %{label: "Docs", url: "ftp://example.com"})
      |> render_submit()

    assert html =~ "Url must be an http:// or https:// URL"
    assert Links.list(filters: [post_id: post.id]) == []

    view
    |> form("#link-form-#{post.id}", %{label: "Docs", url: "https://hexdocs.pm"})
    |> render_submit()

    assert [link] = Links.list(filters: [post_id: post.id])
    assert has_element?(view, ~s(#link-#{link.id} a[href="https://hexdocs.pm"]), "Docs")

    view |> element("#link-#{link.id} button") |> render_click()

    refute has_element?(view, "#link-#{link.id}")
    assert Links.list(filters: [post_id: post.id]) == []
  end

  test "the owner tags a post, making the tag on the way, and untags it", %{conn: conn, scope: scope} do
    post = post_fixture(scope, [], %{body: "Mine"})
    {:ok, view, _html} = live(conn, ~p"/posts")

    view |> form("#tag-form-#{post.id}", %{name: " Elixir "}) |> render_submit()
    view |> form("#tag-form-#{post.id}", %{name: "elixir"}) |> render_submit()

    assert [%{tag: %{name: "Elixir"}} = row] = Tagging.tags_of(post.id)
    assert has_element?(view, "#post-tag-#{row.id}", "Elixir")

    view |> element("#post-tag-#{row.id} button") |> render_click()

    refute has_element?(view, "#post-tag-#{row.id}")
    assert Tagging.tags_of(post.id) == []
    assert [{%{name: "Elixir"}, 0}] = Tagging.tags_with_counts(scope)
  end

  test "a reader sees a post's links and tags, and none of the owner's controls", %{conn: conn, scope: scope} do
    community = joined_community_fixture(scope)
    owner = user_scope_fixture(user_fixture())
    {:ok, _membership} = App.Data.Memberships.join(owner, community.id)
    post = post_fixture(owner, [community.id], %{body: "Theirs"})
    {:ok, _link} = Posting.add_link(owner, post.id, %{"label" => "Docs", "url" => "https://hexdocs.pm"})
    {:ok, _row} = Tagging.tag_post(owner, post, "elixir")

    {:ok, view, _html} = live(conn, ~p"/posts")

    assert has_element?(view, "#posts-#{post.id} a", "Docs")
    assert has_element?(view, "#posts-#{post.id} .badge", "elixir")
    refute has_element?(view, "#posts-#{post.id} button")
    refute has_element?(view, "#link-form-#{post.id}")
  end
end
