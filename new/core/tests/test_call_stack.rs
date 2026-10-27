use std::path::Path;
use std::process::Command;

use code_reviewer_core::analysis::call_trace::TraceStopReason;
use code_reviewer_core::analysis::function_changes::FunctionChangeKind;
use code_reviewer_core::analysis::function_diff::DiffLineKind;
use code_reviewer_core::analysis::model::FunctionId;
use code_reviewer_core::analysis::service::{
    AnalysisService, AnalyzeChangesRequest, GetTestCallStackRequest, ListTestsRequest,
};
use code_reviewer_core::analysis::test_call_stack::TestCallStackOptions;

const ORDER_LIVE_TEST: &str = r#"defmodule ShopWeb.OrderLiveTest do
  use ShopWeb.ConnCase

  test "lists orders", %{conn: conn} do
    {:ok, view, _html} = live(conn, "/orders")
    assert render(view) =~ "Orders"
    ShopWeb.OrderLive.handle_event("refresh", %{}, view)
  end
end
"#;

const ORDERS_TEST: &str = r#"defmodule Shop.OrdersTest do
  use Shop.DataCase

  test "lists" do
    Shop.Orders.list_orders()
  end
end
"#;

const ORDER_LIVE: &str = r#"defmodule ShopWeb.OrderLive do
  use ShopWeb, :live_view

  alias Shop.Orders

  def mount(_params, _session, socket) do
    {:ok, assign(socket, :orders, Orders.list_orders())}
  end

  def handle_event("refresh", _params, socket) do
    {:noreply, assign(socket, :orders, Orders.list_orders())}
  end
end
"#;

const ORDERS: &str = r#"defmodule Shop.Orders do
  import Ecto.Query

  def list_orders do
    Shop.Repo.all(from o in Shop.Order, order_by: o.inserted_at)
  end

  def count_orders do
    list_orders() |> length()
  end
end
"#;

const REPO: &str = r#"defmodule Shop.Repo do
  def all(query), do: query
end
"#;

fn run_git(dir: &Path, args: &[&str]) {
    let status = Command::new("git")
        .args(args)
        .current_dir(dir)
        .env("GIT_AUTHOR_NAME", "t")
        .env("GIT_AUTHOR_EMAIL", "t@example.com")
        .env("GIT_COMMITTER_NAME", "t")
        .env("GIT_COMMITTER_EMAIL", "t@example.com")
        .status()
        .unwrap();
    assert!(status.success(), "git {args:?} failed");
}

fn write(dir: &Path, path: &str, text: &str) {
    let full = dir.join(path);
    std::fs::create_dir_all(full.parent().unwrap()).unwrap();
    std::fs::write(full, text).unwrap();
}

fn commit(repo: &Path, message: &str) {
    run_git(repo, &["add", "-A"]);
    run_git(repo, &["commit", "-q", "-m", message]);
}

/// Two commits. The second changes `Orders.list_orders/0`, adds
/// `Orders.recent_orders/1` to the LiveView mount and touches the test.
fn phoenix_repo_with_two_commits() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    let repo = dir.path();
    run_git(repo, &["init", "-q"]);
    write(
        repo,
        "test/shop_web/live/order_live_test.exs",
        ORDER_LIVE_TEST,
    );
    write(repo, "test/shop/orders_test.exs", ORDERS_TEST);
    write(repo, "lib/shop_web/live/order_live.ex", ORDER_LIVE);
    write(repo, "lib/shop/orders.ex", ORDERS);
    write(repo, "lib/shop/repo.ex", REPO);
    commit(repo, "first");

    write(
        repo,
        "lib/shop/orders.ex",
        &ORDERS.replace(
            "order_by: o.inserted_at)\n  end\n",
            "order_by: [desc: o.inserted_at])\n  end\n\n  def recent_orders(limit) do\n    list_orders() |> Enum.take(limit)\n  end\n",
        ),
    );
    write(
        repo,
        "lib/shop_web/live/order_live.ex",
        &ORDER_LIVE.replace(
            "{:ok, assign(socket, :orders, Orders.list_orders())}",
            "{:ok, assign(socket, :orders, Orders.recent_orders(10))}",
        ),
    );
    write(
        repo,
        "test/shop_web/live/order_live_test.exs",
        &ORDER_LIVE_TEST.replace("=~ \"Orders\"", "=~ \"Recent orders\""),
    );
    commit(repo, "second");
    dir
}

fn comparison(repo: &Path) -> AnalyzeChangesRequest {
    AnalyzeChangesRequest {
        repo_path: repo.to_string_lossy().to_string(),
        head_ref: Some("HEAD".to_string()),
        ..Default::default()
    }
}

#[test]
fn test_tests_are_listed_with_live_view_guess_and_change() {
    let dir = phoenix_repo_with_two_commits();
    let service = AnalysisService::new();

    let response = service
        .list_tests(ListTestsRequest {
            changes: comparison(dir.path()),
        })
        .unwrap();

    let rows: Vec<_> = response
        .tests
        .iter()
        .map(|test| {
            (
                test.function.to_string(),
                test.is_live_view_test,
                test.change,
            )
        })
        .collect();
    assert_eq!(
        rows,
        [
            ("Shop.OrdersTest.test lists/0".to_string(), false, None),
            (
                "ShopWeb.OrderLiveTest.test lists orders/0".to_string(),
                true,
                Some(FunctionChangeKind::Modified)
            ),
        ]
    );
}

#[test]
fn test_call_stack_walks_calls_in_order_and_marks_changed_functions() {
    let dir = phoenix_repo_with_two_commits();
    let service = AnalysisService::new();

    let response = service
        .get_test_call_stack(GetTestCallStackRequest {
            changes: comparison(dir.path()),
            test: FunctionId::new("ShopWeb.OrderLiveTest", "test lists orders", 0),
            options: TestCallStackOptions::default(),
        })
        .unwrap();

    let rows: Vec<_> = response
        .frames
        .iter()
        .map(|frame| {
            (
                frame.depth,
                frame.function.to_string(),
                frame.defined_in_project,
                frame.change,
                frame.repeated,
            )
        })
        .collect();
    // `live/2` and `render/1` are injected by ConnCase, so they resolve to nothing. The
    // handle_event call is a project call. Below it, `assign/3` is outside the project,
    // `recent_orders` is new, `list_orders` is modified and `Enum.take` is outside.
    assert_eq!(
        rows,
        [
            (
                0,
                "ShopWeb.OrderLiveTest.test lists orders/0".to_string(),
                true,
                Some(FunctionChangeKind::Modified),
                false
            ),
            (
                1,
                "ShopWeb.OrderLive.handle_event/3".to_string(),
                true,
                None,
                false
            ),
            (
                2,
                "Shop.Orders.list_orders/0".to_string(),
                true,
                Some(FunctionChangeKind::Modified),
                false
            ),
            (3, "Shop.Repo.all/1".to_string(), true, None, false),
        ]
    );
    assert!(!response.truncated);

    let list_orders = &response.frames[2];
    assert_eq!(list_orders.file_path.as_deref(), Some("lib/shop/orders.ex"));
    assert_eq!(
        list_orders.call_file_path.as_deref(),
        Some("lib/shop_web/live/order_live.ex")
    );
    assert_eq!(list_orders.call_lines, [11]);
    let marks: Vec<_> = list_orders.lines.iter().map(|line| line.kind).collect();
    assert_eq!(
        marks,
        [
            DiffLineKind::Context,
            DiffLineKind::Removed,
            DiffLineKind::Added,
            DiffLineKind::Context
        ]
    );

    let repo_all = &response.frames[3];
    assert!(repo_all
        .lines
        .iter()
        .all(|line| line.kind == DiffLineKind::Context));
    assert!(repo_all
        .lines
        .iter()
        .all(|line| line.old_number.is_some() && line.new_number.is_some()));
}

#[test]
fn test_call_stack_lists_a_function_once_with_its_calls_and_again_as_repeated() {
    let dir = phoenix_repo_with_two_commits();
    write(
        dir.path(),
        "test/shop/orders_test.exs",
        &ORDERS_TEST.replace(
            "    Shop.Orders.list_orders()\n",
            "    Shop.Orders.recent_orders(1)\n    Shop.Orders.count_orders()\n",
        ),
    );
    let service = AnalysisService::new();

    let response = service
        .get_test_call_stack(GetTestCallStackRequest {
            changes: AnalyzeChangesRequest {
                repo_path: dir.path().to_string_lossy().to_string(),
                ..Default::default()
            },
            test: FunctionId::new("Shop.OrdersTest", "test lists", 0),
            options: TestCallStackOptions::default(),
        })
        .unwrap();

    let rows: Vec<_> = response
        .frames
        .iter()
        .map(|frame| {
            (
                frame.depth,
                frame.function.name.as_str(),
                frame.defined_in_project,
                frame.repeated,
            )
        })
        .collect();
    assert_eq!(
        rows,
        [
            (0, "test lists", true, false),
            (1, "recent_orders", true, false),
            (2, "list_orders", true, false),
            (3, "all", true, false),
            (2, "take", false, false),
            (1, "count_orders", true, false),
            (2, "list_orders", true, true),
        ]
    );
    assert_eq!(
        response.frames[0].change,
        Some(FunctionChangeKind::Modified)
    );
    assert_eq!(
        response.frames[6].lines.len(),
        response.frames[2].lines.len()
    );
}

#[test]
fn test_call_stack_stops_at_max_depth() {
    let dir = phoenix_repo_with_two_commits();
    let service = AnalysisService::new();

    let response = service
        .get_test_call_stack(GetTestCallStackRequest {
            changes: comparison(dir.path()),
            test: FunctionId::new("ShopWeb.OrderLiveTest", "test lists orders", 0),
            options: TestCallStackOptions {
                max_depth: Some(1),
                max_frame_count: None,
            },
        })
        .unwrap();

    assert_eq!(response.frames.len(), 2);
    assert_eq!(
        response.frames[1].stop_reason,
        Some(TraceStopReason::MaxDepth)
    );
    assert!(response.truncated);
}

#[test]
fn test_unknown_test_is_an_error() {
    let dir = phoenix_repo_with_two_commits();
    let service = AnalysisService::new();

    let error = service
        .get_test_call_stack(GetTestCallStackRequest {
            changes: comparison(dir.path()),
            test: FunctionId::new("ShopWeb.OrderLiveTest", "test missing", 0),
            options: TestCallStackOptions::default(),
        })
        .unwrap_err();

    assert!(error.to_string().contains("test missing"));
}
