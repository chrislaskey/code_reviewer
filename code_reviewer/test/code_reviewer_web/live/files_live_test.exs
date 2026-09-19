defmodule CodeReviewerWeb.FilesLiveTest do
  use CodeReviewerWeb.ConnCase, async: true

  import Phoenix.LiveViewTest

  # A throwaway repository with two commits: one file modified, one added in
  # a nested directory, one removed.
  setup do
    dir =
      Path.join(System.tmp_dir!(), "code_reviewer_files_#{System.unique_integer([:positive])}")

    File.mkdir_p!(Path.join(dir, "lib/shop/deep"))

    git = fn args -> {_, 0} = System.cmd("git", args, cd: dir, stderr_to_stdout: true) end
    git.(["init", "-q"])
    git.(["config", "user.email", "test@example.com"])
    git.(["config", "user.name", "Test"])

    File.write!(
      Path.join(dir, "lib/shop.ex"),
      "defmodule Shop do\n  def total(cart), do: cart\nend\n"
    )

    File.write!(Path.join(dir, "README.md"), "# Shop\n")

    git.(["add", "."])
    git.(["commit", "-q", "-m", "first"])

    File.write!(Path.join(dir, "lib/shop.ex"), """
    defmodule Shop do
      def total(cart), do: Enum.sum(cart)

      def add(cart, item), do: [item | cart]
    end
    """)

    File.write!(Path.join(dir, "lib/shop/deep/cart.ex"), "defmodule Shop.Cart do\nend\n")
    File.rm!(Path.join(dir, "README.md"))

    git.(["add", "-A", "."])
    git.(["commit", "-q", "-m", "Second commit"])

    on_exit(fn -> File.rm_rf!(dir) end)
    %{repo: dir}
  end

  test "renders one card per file with its hunks", %{conn: conn, repo: repo} do
    {:ok, view, _html} = live(conn, ~p"/files?#{%{repo: repo, rev: "HEAD"}}")

    assert has_element?(view, "#files-loading")
    render_async(view, 5_000)

    assert has_element?(view, "#files-summary", "Second commit")
    assert has_element?(view, "#files-summary", "3 files changed")
    assert has_element?(view, "#view-nav a[data-section='files'][aria-current='page']")

    card = "#file-cards article[data-path='lib/shop.ex']"
    assert has_element?(view, card <> "[data-verb='updated']")
    assert has_element?(view, card <> " [data-file-path]", "lib/shop.ex")
    assert has_element?(view, card <> " [data-hunk] [data-kind='del'][data-old='2']", "do: cart")

    assert has_element?(
             view,
             card <> " [data-hunk] [data-kind='add'][data-new='2']",
             "Enum.sum(cart)"
           )

    assert has_element?(view, card <> " [data-hunk]", "@@ -1,3 +1,5 @@")

    assert has_element?(
             view,
             "#file-cards article[data-path='lib/shop/deep/cart.ex'][data-verb='created']"
           )

    assert has_element?(view, "#file-cards article[data-path='README.md'][data-verb='deleted']")
  end

  test "the view carries the vim hook, a status line and stable line ids", %{
    conn: conn,
    repo: repo
  } do
    {:ok, view, _html} = live(conn, ~p"/files?#{%{repo: repo, rev: "HEAD"}}")
    render_async(view, 5_000)

    assert has_element?(view, "#files-view[phx-hook='VimFiles']")
    assert has_element?(view, "#files-statusline [data-vim-total]", "3")

    id = "f-" <> Base.url_encode64("lib/shop.ex", padding: false)

    assert has_element?(
             view,
             "#file-cards article##{id} [data-hunk] [data-line='0'][data-id='#{id}:0:0'][data-kind='context']"
           )

    assert has_element?(view, "#file-tree a[data-file='#{id}']")
  end

  test "enter on a diff line reports the line", %{conn: conn, repo: repo} do
    {:ok, view, _html} = live(conn, ~p"/files?#{%{repo: repo, rev: "HEAD"}}")
    render_async(view, 5_000)

    view
    |> element("#files-view")
    |> render_hook("activate", %{
      "id" => "x",
      "path" => "lib/shop.ex",
      "line" => %{"kind" => "add", "old" => nil, "new" => "2"}
    })

    assert has_element?(view, "#selected-line", "lib/shop.ex")
    assert has_element?(view, "#selected-line", "line +2")
  end

  test "the tree nests directories, compacting single-child chains", %{conn: conn, repo: repo} do
    {:ok, view, _html} = live(conn, ~p"/files?#{%{repo: repo, rev: "HEAD"}}")
    render_async(view, 5_000)

    # lib/ holds shop.ex and shop/deep/ which is compacted into one directory node
    assert has_element?(view, "#file-tree details > summary", "lib")
    assert has_element?(view, "#file-tree details details > summary", "shop/deep")
    assert has_element?(view, "#file-tree details details a[data-file]", "cart.ex")
    assert has_element?(view, "#file-tree > ul > li > a[data-file]", "README.md")

    # the tree links to the card by id
    id = "f-" <> Base.url_encode64("lib/shop.ex", padding: false)
    assert has_element?(view, "#file-tree a[href='##{id}']")
    assert has_element?(view, "#file-cards article##{id}")
  end

  test "collapsing and marking viewed update one card", %{conn: conn, repo: repo} do
    {:ok, view, _html} = live(conn, ~p"/files?#{%{repo: repo, rev: "HEAD"}}")
    render_async(view, 5_000)

    card = "#file-cards article[data-path='lib/shop.ex']"
    assert has_element?(view, card <> " [data-diff]")

    view |> element(card <> " button[phx-click='toggle_collapsed']") |> render_click()
    assert has_element?(view, card <> "[data-collapsed]")
    refute has_element?(view, card <> " [data-diff]")

    view |> element(card <> " button[phx-click='toggle_collapsed']") |> render_click()
    refute has_element?(view, card <> "[data-collapsed]")
    assert has_element?(view, card <> " [data-diff]")

    # viewed folds the card and counts in the header
    view |> element(card <> " input[phx-click='toggle_viewed']") |> render_click()
    assert has_element?(view, card <> "[data-viewed][data-collapsed]")
    assert has_element?(view, "#viewed-progress [data-viewed-count]", "1")

    view |> element(card <> " input[phx-click='toggle_viewed']") |> render_click()
    refute has_element?(view, card <> "[data-viewed]")
    refute has_element?(view, card <> "[data-collapsed]")
    assert has_element?(view, "#viewed-progress [data-viewed-count]", "0")
  end

  test "expand all and collapse all", %{conn: conn, repo: repo} do
    {:ok, view, _html} = live(conn, ~p"/files?#{%{repo: repo, rev: "HEAD"}}")
    render_async(view, 5_000)

    view |> element("#file-tree button[phx-click='collapse_all']") |> render_click()
    refute has_element?(view, "#file-cards article:not([data-collapsed])")

    view |> element("#file-tree button[phx-click='expand_all']") |> render_click()
    refute has_element?(view, "#file-cards article[data-collapsed]")
  end

  test "submitting the source form patches the url and reloads", %{conn: conn, repo: repo} do
    {:ok, view, _html} = live(conn, ~p"/files")

    view
    |> form("#source-form", %{"repo" => repo, "rev" => "HEAD"})
    |> render_submit()

    assert_patch(view, ~p"/files?#{%{repo: repo, rev: "HEAD"}}")
    render_async(view, 5_000)
    assert has_element?(view, "#file-cards article[data-path='lib/shop.ex']")
  end

  test "a bad repository path shows an error state", %{conn: conn} do
    {:ok, view, _html} = live(conn, ~p"/files?#{%{repo: "/definitely/not/here", rev: "HEAD"}}")
    render_async(view, 5_000)

    assert has_element?(view, "#files-error", "not a directory")
    refute has_element?(view, "#files-view")
  end
end
