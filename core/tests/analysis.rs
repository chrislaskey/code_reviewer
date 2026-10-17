use std::path::Path;
use std::process::Command;

use code_reviewer_core::analysis::call_trace::{get_call_trace, CallTraceOptions, TraceStopReason};
use code_reviewer_core::analysis::changed_functions::list_changed_functions;
use code_reviewer_core::analysis::external_calls::list_external_calls_for_changed_functions;
use code_reviewer_core::analysis::function_source::get_function_source;
use code_reviewer_core::analysis::model::FunctionId;
use code_reviewer_core::analysis::project_index::{
    build_project_index_from_sources, CallDirection, ProjectIndexCache, SourceFile,
};
use code_reviewer_core::git::{list_changed_line_ranges, ListChangedLineRangesRequest};

const ORDERS: &str = r#"defmodule Shop.Orders do
  alias Shop.Billing

  @doc "Places an order."
  def place(order) do
    order
    |> validate()
    |> Billing.charge(:card)
  end

  defp validate(order), do: Shop.Audit.record(order)

  def loop_a(x), do: loop_b(x)
  def loop_b(x), do: loop_a(x)
end
"#;

const BILLING: &str = r#"defmodule Shop.Billing do
  def charge(order, method) do
    Shop.Audit.record({order, method})
    Stripe.Charge.create(order)
  end
end
"#;

const AUDIT: &str = r#"defmodule Shop.Audit do
  def record(entry), do: entry
end
"#;

const ORDERS_TEST: &str = r#"defmodule Shop.OrdersTest do
  test "places" do
    Shop.Orders.place(%{})
  end
end
"#;

fn sample_index() -> code_reviewer_core::analysis::project_index::ProjectIndex {
    let source = |path: &str, text: &str| SourceFile {
        path: path.to_string(),
        text: text.to_string(),
    };
    build_project_index_from_sources(vec![
        source("lib/shop/orders.ex", ORDERS),
        source("lib/shop/billing.ex", BILLING),
        source("lib/shop/audit.ex", AUDIT),
        source("test/orders_test.exs", ORDERS_TEST),
        source("README.md", "not code"),
    ])
}

fn id(module: &str, name: &str, arity: usize) -> FunctionId {
    FunctionId::new(module, name, arity)
}

#[test]
fn test_one_level_external_calls_cover_both_directions() {
    let index = sample_index();
    let calls = code_reviewer_core::analysis::external_calls::list_external_calls_for_function(
        &index,
        &id("Shop.Billing", "charge", 2),
    )
    .unwrap();

    let callers: Vec<String> = calls
        .calls_in
        .iter()
        .map(|l| l.function.to_string())
        .collect();
    assert_eq!(callers, vec!["Shop.Orders.place/1"]);

    let callees: Vec<(String, bool)> = calls
        .calls_out
        .iter()
        .map(|l| (l.function.to_string(), l.defined_in_project))
        .collect();
    assert_eq!(
        callees,
        vec![
            ("Shop.Audit.record/1".to_string(), true),
            ("Stripe.Charge.create/1".to_string(), false),
        ]
    );
}

#[test]
fn test_external_calls_leave_out_same_module_calls() {
    let index = sample_index();
    let calls = code_reviewer_core::analysis::external_calls::list_external_calls_for_function(
        &index,
        &id("Shop.Orders", "loop_a", 1),
    )
    .unwrap();
    assert!(calls.calls_in.is_empty() && calls.calls_out.is_empty());
}

#[test]
fn test_incoming_calls_include_tests_and_pipe_arity() {
    let index = sample_index();
    let incoming = index.list_incoming_call_links(&id("Shop.Orders", "place", 1));
    assert_eq!(incoming[0].function.name, "test places");

    // `|> Billing.charge(:card)` is charge/2
    let charge = index.list_incoming_call_links(&id("Shop.Billing", "charge", 2));
    assert_eq!(charge[0].call_lines, vec![8]);
}

#[test]
fn test_full_trace_follows_local_and_external_calls() {
    let index = sample_index();
    let response = get_call_trace(
        &index,
        &id("Shop.Orders", "place", 1),
        CallTraceOptions {
            direction: CallDirection::OutgoingCalls,
            max_depth: None,
            max_node_count: None,
        },
    )
    .unwrap();

    let root = response.root;
    let names: Vec<String> = root
        .children
        .iter()
        .map(|c| c.function.to_string())
        .collect();
    assert_eq!(
        names,
        vec!["Shop.Billing.charge/2", "Shop.Orders.validate/1"]
    );
    let validate = &root.children[1];
    assert!(!validate.crosses_module_boundary);
    assert_eq!(
        validate.children[0].function.to_string(),
        "Shop.Audit.record/1"
    );
    assert!(!response.truncated);
}

#[test]
fn test_trace_marks_cycles_and_depth_limits() {
    let index = sample_index();
    let trace = |max_depth| {
        get_call_trace(
            &index,
            &id("Shop.Orders", "loop_a", 1),
            CallTraceOptions {
                direction: CallDirection::OutgoingCalls,
                max_depth,
                max_node_count: None,
            },
        )
        .unwrap()
    };

    let full = trace(None);
    let loop_b = &full.root.children[0];
    assert_eq!(loop_b.children[0].stop_reason, Some(TraceStopReason::Cycle));

    let shallow = trace(Some(1));
    assert!(shallow.truncated);
    assert_eq!(
        shallow.root.children[0].stop_reason,
        Some(TraceStopReason::MaxDepth)
    );
}

#[test]
fn test_incoming_trace_walks_up_to_the_tests() {
    let index = sample_index();
    let response = get_call_trace(
        &index,
        &id("Shop.Audit", "record", 1),
        CallTraceOptions {
            direction: CallDirection::IncomingCalls,
            max_depth: None,
            max_node_count: None,
        },
    )
    .unwrap();

    let billing = response
        .root
        .children
        .iter()
        .find(|c| c.function.module == "Shop.Billing")
        .unwrap();
    assert_eq!(billing.children[0].function.name, "place");
    assert_eq!(billing.children[0].children[0].function.name, "test places");
}

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

#[test]
fn test_changed_function_flow_on_a_real_git_repo_with_a_warm_cache() {
    let dir = tempfile::tempdir().unwrap();
    let repo = dir.path();
    run_git(repo, &["init", "-q"]);
    write(repo, "lib/shop/orders.ex", ORDERS);
    write(repo, "lib/shop/billing.ex", BILLING);
    write(repo, "lib/shop/audit.ex", AUDIT);
    run_git(repo, &["add", "."]);
    run_git(repo, &["commit", "-q", "-m", "init"]);

    // Edit one line inside Billing.charge/2 only.
    write(
        repo,
        "lib/shop/billing.ex",
        &BILLING.replace(
            "Stripe.Charge.create(order)",
            "Stripe.Charge.create(order, method)",
        ),
    );

    let cache = ProjectIndexCache::new();
    let index = cache.get_current_project_index(repo).unwrap();
    let ranges = list_changed_line_ranges(ListChangedLineRangesRequest {
        repo_path: repo.to_string_lossy().to_string(),
        base_ref: None,
        head_ref: None,
    })
    .unwrap();
    let changed = list_changed_functions(&index, &ranges.files);

    assert_eq!(changed.len(), 1);
    assert_eq!(changed[0].function, id("Shop.Billing", "charge", 2));
    assert_eq!(changed[0].changed_lines, vec![4]);

    let external = list_external_calls_for_changed_functions(&index, &changed).unwrap();
    assert_eq!(external[0].calls_in[0].function.name, "place");

    // Full function source, not just lines around the change.
    let source = get_function_source(&index, &changed[0].function).unwrap();
    assert_eq!((source.start_line, source.end_line), (2, 5));
    assert_eq!(source.lines.len(), 4);
    assert!(source.lines[0].text.contains("def charge"));

    // Nothing changed on disk: the very same index comes back.
    let again = cache.get_current_project_index(repo).unwrap();
    assert!(std::sync::Arc::ptr_eq(&index, &again));

    // A new file: the index is rebuilt and includes it.
    write(
        repo,
        "lib/shop/extra.ex",
        "defmodule Shop.Extra do\n  def one, do: 1\nend\n",
    );
    let rebuilt = cache.get_current_project_index(repo).unwrap();
    assert!(!std::sync::Arc::ptr_eq(&index, &rebuilt));
    assert!(rebuilt.get_function(&id("Shop.Extra", "one", 0)).is_some());

    // Untracked files count as fully changed.
    let ranges = list_changed_line_ranges(ListChangedLineRangesRequest {
        repo_path: repo.to_string_lossy().to_string(),
        base_ref: None,
        head_ref: None,
    })
    .unwrap();
    let changed = list_changed_functions(&rebuilt, &ranges.files);
    assert!(changed.iter().any(|c| c.function.module == "Shop.Extra"));
}

mod revisions {
    use super::*;
    use code_reviewer_core::analysis::function_changes::{FunctionChange, FunctionChangeKind};
    use code_reviewer_core::analysis::service::{
        AnalysisService, AnalyzeChangesRequest, GetCallTraceRequest, GetFunctionSourceRequest,
    };

    fn commit(repo: &Path, message: &str) {
        run_git(repo, &["add", "-A"]);
        run_git(repo, &["commit", "-q", "-m", message]);
    }

    /// Two commits. The second modifies `Billing.charge/2`, renames
    /// `Audit.record/1` to `Audit.log/1` (callers keep the old name) and
    /// deletes `Orders.loop_a/1` and `loop_b/1`.
    fn repo_with_two_commits() -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        let repo = dir.path();
        run_git(repo, &["init", "-q"]);
        write(repo, "lib/shop/orders.ex", ORDERS);
        write(repo, "lib/shop/billing.ex", BILLING);
        write(repo, "lib/shop/audit.ex", AUDIT);
        commit(repo, "first");

        write(
            repo,
            "lib/shop/billing.ex",
            &BILLING.replace(
                "Stripe.Charge.create(order)",
                "Stripe.Charge.create(order, method)",
            ),
        );
        write(repo, "lib/shop/audit.ex", &AUDIT.replace("record", "log"));
        let orders = ORDERS.replace(
            "  def loop_a(x), do: loop_b(x)\n  def loop_b(x), do: loop_a(x)\n",
            "",
        );
        write(repo, "lib/shop/orders.ex", &orders);
        commit(repo, "second");
        dir
    }

    fn analyze(
        service: &AnalysisService,
        repo: &Path,
        base_ref: Option<&str>,
        head_ref: Option<&str>,
    ) -> Vec<FunctionChange> {
        service
            .analyze_changes(AnalyzeChangesRequest {
                repo_path: repo.to_string_lossy().to_string(),
                base_ref: base_ref.map(str::to_string),
                head_ref: head_ref.map(str::to_string),
            })
            .unwrap()
            .changes
    }

    fn summary(changes: &[FunctionChange]) -> Vec<(FunctionChangeKind, String)> {
        let mut rows: Vec<_> = changes
            .iter()
            .map(|change| (change.kind, change.function.to_string()))
            .collect();
        rows.sort_by(|a, b| a.1.cmp(&b.1));
        rows
    }

    #[test]
    fn test_one_commit_reports_modified_renamed_and_removed_functions() {
        let dir = repo_with_two_commits();
        let service = AnalysisService::new();
        let changes = analyze(&service, dir.path(), None, Some("HEAD"));

        use FunctionChangeKind::*;
        assert_eq!(
            summary(&changes),
            vec![
                (Renamed, "Shop.Audit.log/1".to_string()),
                (Modified, "Shop.Billing.charge/2".to_string()),
                (Removed, "Shop.Orders.loop_a/1".to_string()),
                (Removed, "Shop.Orders.loop_b/1".to_string()),
            ]
        );

        let modified = changes.iter().find(|c| c.kind == Modified).unwrap();
        assert_eq!(modified.changed_lines, vec![4]);
        // Level 1 calls on the new side.
        assert_eq!(modified.external_calls.calls_in[0].function.name, "place");
    }

    #[test]
    fn test_rename_reports_old_callers_and_callers_that_still_use_the_old_name() {
        let dir = repo_with_two_commits();
        let changes = analyze(&AnalysisService::new(), dir.path(), None, Some("HEAD"));
        let renamed = changes
            .iter()
            .find(|c| c.kind == FunctionChangeKind::Renamed)
            .unwrap();

        assert_eq!(
            renamed.previous_function,
            Some(id("Shop.Audit", "record", 1))
        );
        let old_callers: Vec<_> = renamed
            .previous_calls_in
            .iter()
            .map(|l| l.function.to_string())
            .collect();
        assert_eq!(
            old_callers,
            vec!["Shop.Billing.charge/2", "Shop.Orders.validate/1"]
        );
        // Billing.charge still says `Shop.Audit.record`. Orders.validate does too.
        assert_eq!(renamed.stale_callers.len(), 2);
    }

    #[test]
    fn test_revision_range_matches_the_single_commit_and_sources_read_from_the_old_side() {
        let dir = repo_with_two_commits();
        let service = AnalysisService::new();
        let by_commit = analyze(&service, dir.path(), None, Some("HEAD"));
        let by_range = analyze(&service, dir.path(), Some("HEAD~1"), Some("HEAD"));
        assert_eq!(by_commit, by_range);

        let removed = by_range
            .iter()
            .find(|c| c.function.name == "loop_a")
            .unwrap();
        assert!(removed.changed_lines.is_empty());

        // The function is gone from HEAD but its source is readable at the old revision.
        let source = service
            .get_function_source(GetFunctionSourceRequest {
                repo_path: dir.path().to_string_lossy().to_string(),
                revision: Some("HEAD~1".to_string()),
                function: removed.function.clone(),
            })
            .unwrap();
        assert!(source.lines[0].text.contains("def loop_a"));

        let missing = service.get_function_source(GetFunctionSourceRequest {
            repo_path: dir.path().to_string_lossy().to_string(),
            revision: None,
            function: removed.function.clone(),
        });
        assert!(missing.is_err());
    }

    #[test]
    fn test_call_trace_runs_on_a_past_revision() {
        let dir = repo_with_two_commits();
        let response = AnalysisService::new()
            .get_call_trace(GetCallTraceRequest {
                repo_path: dir.path().to_string_lossy().to_string(),
                revision: Some("HEAD~1".to_string()),
                function: id("Shop.Audit", "record", 1),
                direction: CallDirection::IncomingCalls,
                max_depth: None,
                max_node_count: None,
            })
            .unwrap();
        assert_eq!(response.root.children.len(), 2);
    }

    #[test]
    fn test_working_tree_changes_are_compared_with_head() {
        let dir = repo_with_two_commits();
        let repo = dir.path();
        write(
            repo,
            "lib/shop/audit.ex",
            "defmodule Shop.Audit do\n  def log(entry), do: {:ok, entry}\n  def extra, do: 1\nend\n",
        );

        let changes = analyze(&AnalysisService::new(), repo, None, None);
        use FunctionChangeKind::*;
        assert_eq!(
            summary(&changes),
            vec![
                (Added, "Shop.Audit.extra/0".to_string()),
                (Modified, "Shop.Audit.log/1".to_string()),
            ]
        );
    }

    #[test]
    fn test_moving_a_file_without_editing_it_changes_no_functions() {
        let dir = repo_with_two_commits();
        let repo = dir.path();
        // Unstaged move: the old path is deleted and the new one is untracked.
        std::fs::create_dir_all(repo.join("lib/shop/moved")).unwrap();
        std::fs::rename(
            repo.join("lib/shop/billing.ex"),
            repo.join("lib/shop/moved/billing.ex"),
        )
        .unwrap();

        let changes = analyze(&AnalysisService::new(), repo, None, None);
        assert!(changes.is_empty(), "{changes:?}");

        // Committed move: same result between the two commits.
        commit(repo, "move");
        let changes = analyze(&AnalysisService::new(), repo, None, Some("HEAD"));
        assert!(changes.is_empty(), "{changes:?}");
    }

    #[test]
    fn test_bad_revisions_are_rejected() {
        let dir = repo_with_two_commits();
        let service = AnalysisService::new();
        for bad in ["--output=/tmp/x", "no-such-branch", ""] {
            let result = service.analyze_changes(AnalyzeChangesRequest {
                repo_path: dir.path().to_string_lossy().to_string(),
                base_ref: Some(bad.to_string()),
                head_ref: None,
            });
            assert!(result.is_err(), "{bad:?} should be rejected");
        }
    }
}
