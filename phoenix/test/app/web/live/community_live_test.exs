defmodule App.Web.CommunityLiveTest do
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

  test "shows a community to a member", %{conn: conn, scope: scope} do
    community = joined_community_fixture(scope)

    {:ok, view, _html} = live(conn, ~p"/communities/#{community}")

    assert has_element?(view, "#community-page h1", community.name)
  end

  test "refuses a non-member", %{conn: conn} do
    community = community_fixture()

    assert {:error, {:redirect, %{to: "/communities", flash: flash}}} =
             live(conn, ~p"/communities/#{community}")

    assert flash["error"] =~ "not a member"
  end

  test "refuses a member who left", %{conn: conn, scope: scope} do
    community = joined_community_fixture(scope)
    {:ok, _membership} = Memberships.leave(scope, community.id)

    assert {:error, {:redirect, %{to: "/communities"}}} =
             live(conn, ~p"/communities/#{community}")
  end

  test "shows the community's feed and removes the user's own post from it",
       %{conn: conn, scope: scope} do
    community = joined_community_fixture(scope)
    post = App.Data.PostsFixtures.post_fixture(scope, [community.id], %{body: "In here"})
    elsewhere = App.Data.PostsFixtures.post_fixture(scope, [], %{body: "Not here"})

    {:ok, view, _html} = live(conn, ~p"/communities/#{community}")

    assert has_element?(view, "#posts-#{post.id}", "In here")
    refute has_element?(view, "#posts-#{elsewhere.id}")

    view |> element("#posts-#{post.id} button", "Remove") |> render_click()

    refute has_element?(view, "#posts-#{post.id}")
    assert App.Data.Posting.feed(App.Web.Scope.put_community(scope, community)) == []
  end

  test "leaves from the page", %{conn: conn, scope: scope} do
    community = joined_community_fixture(scope)
    {:ok, view, _html} = live(conn, ~p"/communities/#{community}")

    view |> element("#leave-community") |> render_click()

    assert_redirect(view, ~p"/communities")
    refute Memberships.member?(scope, community.id)
  end

  describe "comments" do
    setup %{scope: scope} do
      community = joined_community_fixture(scope)
      owner = user_scope_fixture(user_fixture())
      {:ok, _membership} = Memberships.join(owner, community.id)
      post = App.Data.PostsFixtures.post_fixture(owner, [community.id], %{body: "Theirs"})
      %{community: community, owner: owner, post: post}
    end

    test "a member comments on a post, a bad one shows its error at once, and they delete their own",
         %{conn: conn, community: community, post: post} do
      {:ok, view, _html} = live(conn, ~p"/communities/#{community}")

      html = view |> form("#comment-form-#{post.id}", %{body: " "}) |> render_submit()
      assert html =~ "Body can&#39;t be blank"

      view |> form("#comment-form-#{post.id}", %{body: "Nice post"}) |> render_submit()
      assert [comment] = thread(community, post)
      assert has_element?(view, "#comment-#{comment.id}", "Nice post")

      view |> element("#comment-#{comment.id} button") |> render_click()
      refute has_element?(view, "#comment-#{comment.id}")
      assert thread(community, post) == []
    end

    test "only the commenter and the post's owner see a delete button",
         %{conn: conn, community: community, owner: owner, post: post} do
      other = user_scope_fixture(user_fixture())
      {:ok, _membership} = Memberships.join(other, community.id)

      {:ok, theirs} =
        App.Data.Commenting.comment(App.Web.Scope.put_community(other, community), post, "Theirs")

      {:ok, view, _html} = live(conn, ~p"/communities/#{community}")
      assert has_element?(view, "#comment-#{theirs.id}", "Theirs")
      refute has_element?(view, "#comment-#{theirs.id} button")

      owner_conn = log_in_user(build_conn(), owner.user)
      {:ok, owner_view, _html} = live(owner_conn, ~p"/communities/#{community}")
      owner_view |> element("#comment-#{theirs.id} button") |> render_click()
      refute has_element?(owner_view, "#comment-#{theirs.id}")
    end
  end

  defp thread(community, post), do: Map.get(App.Data.Commenting.threads(community.id, [post.id]), post.id, [])
end
