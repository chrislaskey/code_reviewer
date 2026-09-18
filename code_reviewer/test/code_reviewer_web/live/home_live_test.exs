defmodule CodeReviewerWeb.HomeLiveTest do
  use CodeReviewerWeb.ConnCase, async: true

  import Phoenix.LiveViewTest

  # The review shells out to git; give it more than the 100 ms default.

  # A throwaway repository with two commits, so the page has something to show.
  setup do
    dir = Path.join(System.tmp_dir!(), "code_reviewer_live_#{System.unique_integer([:positive])}")
    File.mkdir_p!(Path.join(dir, "lib"))

    git = fn args -> {_, 0} = System.cmd("git", args, cd: dir, stderr_to_stdout: true) end
    git.(["init", "-q"])
    git.(["config", "user.email", "test@example.com"])
    git.(["config", "user.name", "Test"])

    File.write!(
      Path.join(dir, "lib/shop.ex"),
      "defmodule Shop do\n  def total(cart), do: cart\nend\n"
    )

    git.(["add", "."])
    git.(["commit", "-q", "-m", "first"])

    File.write!(Path.join(dir, "lib/shop.ex"), """
    defmodule Shop do
      def total(cart), do: Enum.sum(cart)

      def add(cart, item), do: [item | cart]

      defp log(msg), do: msg
    end
    """)

    git.(["add", "."])
    git.(["commit", "-q", "-m", "Add and log"])

    on_exit(fn -> File.rm_rf!(dir) end)
    %{repo: dir}
  end

  test "renders the function index for a revision", %{conn: conn, repo: repo} do
    {:ok, view, _html} = live(conn, ~p"/?#{%{repo: repo, rev: "HEAD"}}")

    assert has_element?(view, "#index-loading")
    render_async(view, 5_000)

    assert has_element?(view, "#function-index")
    assert has_element?(view, "#index-summary", "Add and log")

    # one module row, with its function rows pointing back at it
    assert has_element?(
             view,
             "#function-rows tr[data-kind='module'][data-module='Shop'] [data-column='verb'] [title='Updated']",
             "U"
           )

    assert has_element?(
             view,
             "#function-rows tr[data-kind='module'][data-module='Shop'] [data-column='location']",
             "lib/shop.ex:1"
           )

    assert has_element?(
             view,
             "#function-rows tr[data-kind='function'][data-parent^='m-'][data-function='add/2']"
           )

    # a fold chevron and a count, only on modules that have functions

    assert has_element?(view, "#function-rows tr[data-module='Shop'] [data-chevron='closed']")

    assert has_element?(view, "#function-rows tr[data-module='Shop'] [data-function-count]", "3")

    assert has_element?(
             view,
             "#function-rows tr[data-kind='module'][data-module='Shop'] [data-column='stats']",
             "+5"
           )

    assert has_element?(
             view,
             "#function-rows tr[data-function='total/1'] [data-column='verb'] [title='Updated']",
             "U"
           )

    assert has_element?(
             view,
             "#function-rows tr[data-function='add/2'] [data-column='verb'] [title='Created']",
             "C"
           )

    assert has_element?(
             view,
             "#function-rows tr[data-function='log/1'] [data-column='function']",
             "defp log/1"
           )

    assert has_element?(
             view,
             "#function-rows tr[data-function='add/2'] [data-column='function']",
             "def add/2"
           )

    assert has_element?(view, "#vim-statusline")
  end

  test "enter on a cell reports the selection", %{conn: conn, repo: repo} do
    {:ok, view, _html} = live(conn, ~p"/?#{%{repo: repo, rev: "HEAD"}}")
    render_async(view, 5_000)

    view
    |> element("#function-index")
    |> render_hook("activate", %{
      "id" => "x",
      "column" => "function",
      "module" => "Shop",
      "function" => "add/2"
    })

    assert has_element?(view, "#selected-cell", "Shop.add/2")
  end

  test "enter on a function's module name streams its diff row in and out", %{
    conn: conn,
    repo: repo
  } do
    {:ok, view, _html} = live(conn, ~p"/?#{%{repo: repo, rev: "HEAD"}}")
    render_async(view, 5_000)

    {:ok, review} = CodeReviewer.review_git(repo, from: "HEAD~1", to: "HEAD")

    %{id: id} =
      review |> CodeReviewer.FunctionIndex.rows() |> Enum.find(&(&1.function == "total/1"))

    view |> element("#function-index") |> render_hook("toggle_diff", %{"id" => id})

    assert has_element?(view, "#function-rows tr[data-kind='diff'][data-parent='#{id}']")

    assert has_element?(
             view,
             "#function-rows tr[data-kind='diff'] [data-column='diff']",
             "def total(cart), do: Enum.sum(cart)"
           )

    assert has_element?(
             view,
             "#function-rows tr[data-kind='diff'] [data-column='diff']",
             "def total(cart), do: cart"
           )

    view |> element("#function-index") |> render_hook("toggle_diff", %{"id" => id})
    refute has_element?(view, "#function-rows tr[data-kind='diff']")
  end

  test "submitting the source form patches the url and reloads", %{conn: conn, repo: repo} do
    {:ok, view, _html} = live(conn, ~p"/")

    view
    |> form("#source-form", %{"repo" => repo, "rev" => "HEAD"})
    |> render_submit()

    assert_patch(view)
    render_async(view, 5_000)
    assert has_element?(view, "#function-rows tr[data-function='add/2']")
  end

  test "a bad repository path shows an error state", %{conn: conn} do
    {:ok, view, _html} = live(conn, ~p"/?#{%{repo: "/definitely/not/here", rev: "HEAD"}}")
    render_async(view, 5_000)

    assert has_element?(view, "#index-error", "not a directory")
    refute has_element?(view, "#function-index")
  end
end
