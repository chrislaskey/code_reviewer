import type { GroupSummary } from "../api/types";
import {
  EXAMPLE_FILES,
  calleePath,
  type ChangeKind,
  type ChangedFunction,
  type ExampleCallee,
  type ExampleFile,
} from "./data";

/*
 * Hardcoded groups for the design explorations. Nothing here comes from the
 * backend. `EXAMPLE_GROUPS` has the shape of `GroupMatcher::list_groups` and
 * `group_ids` on each file has the shape of `list_group_ids_for_file`, so the
 * winning design can be wired to the real thing later. Which file lands in
 * which group is written out by hand here; in the app Rust decides.
 */

/** The id of the row that rolls up every group. Mirrors the core constant. */
export const ALL_CHANGES_ID = "@all";

/** The ungrouped group is implicit and only exists next to real groups. */
export const UNGROUPED_ID = "ungrouped";

/** A changed file with the ids of every group that holds it. */
export type GroupedFile = ExampleFile & { group_ids: string[] };

/** What `list_groups` returns for a config with these groups. */
export const EXAMPLE_GROUPS: GroupSummary[] = [
  { id: "data", label: "Data" },
  { id: "business", label: "Business logic" },
  { id: "controllers", label: "Controllers" },
  { id: "live", label: "LiveViews" },
  { id: "tests", label: "Tests" },
  { id: UNGROUPED_ID, label: "Default group" },
];

/* -- Example files ------------------------------------------------------------ */

function fn(
  name: string,
  arity: number,
  kind: ChangeKind,
  added_lines: number,
  removed_lines: number,
): ChangedFunction {
  return { name, arity, kind, added_lines, removed_lines };
}

function call(module: string, name: string, arity: number, from: string): ExampleCallee {
  const callee: ExampleCallee = {
    module,
    name,
    arity,
    origin: "project",
    from,
    call_count: 1,
    diff: "same",
  };
  return { ...callee, path: calleePath(callee) };
}

function extraFile(
  id: string,
  path: string,
  module: string,
  functions: ChangedFunction[],
  callees: ExampleCallee[] = [],
  coverage_percent?: number,
): ExampleFile {
  return { id, path, module, coverage_percent, functions, callers: [], callees };
}

function byId(id: string): ExampleFile {
  const file = EXAMPLE_FILES.find((candidate) => candidate.id === id);
  if (!file) throw new Error(`Missing example file ${id}`);
  return file;
}

function withCalls(file: ExampleFile, extra: ExampleCallee[]): ExampleFile {
  return { ...file, callees: [...file.callees, ...extra] };
}

const FILES_WITH_GROUPS: [ExampleFile, string[]][] = [
  [byId("controller"), ["controllers"]],
  [
    withCalls(byId("orders"), [
      call("Shop.Repo.Queries", "open_orders", 0, "create_order/2"),
      call("Shop.Schemas.Order", "changeset", 2, "create_order/2"),
    ]),
    ["business"],
  ],
  [byId("inventory"), ["business"]],
  [byId("pricing"), ["business"]],
  [
    extraFile(
      "schema_order",
      "lib/shop/schemas/order.ex",
      "Shop.Schemas.Order",
      [fn("changeset", 2, "modified", 8, 2), fn("status_values", 0, "added", 6, 0)],
      [],
      88,
    ),
    ["data"],
  ],
  [
    extraFile(
      "queries",
      "lib/shop/repo/queries.ex",
      "Shop.Repo.Queries",
      [
        fn("open_orders", 0, "modified", 5, 3),
        fn("expired_carts", 0, "added", 11, 0),
        fn("old_totals", 1, "removed", 0, 7),
      ],
      [],
      64,
    ),
    ["data"],
  ],
  [
    extraFile(
      "order_live",
      "lib/shop_web/live/order_live.ex",
      "ShopWeb.OrderLive",
      [fn("handle_event", 3, "modified", 14, 5)],
      [
        call("Shop.Orders", "create_order", 2, "handle_event/3"),
        call("Shop.Orders", "cancel_order", 1, "handle_event/3"),
      ],
      70,
    ),
    ["live"],
  ],
  [
    extraFile(
      "cart_live",
      "lib/shop_web/live/cart_live.ex",
      "ShopWeb.CartLive",
      [fn("render", 1, "modified", 4, 4), fn("mount", 3, "modified", 2, 1)],
      [call("Shop.Pricing", "apply_discount", 2, "render/1")],
      52,
    ),
    ["live"],
  ],
  [
    extraFile(
      "orders_test",
      "test/shop/orders_test.exs",
      "Shop.OrdersTest",
      [
        fn("test creates an order", 0, "modified", 6, 2),
        fn("test cancels an order", 0, "added", 12, 0),
        fn("test totals a legacy order", 0, "removed", 0, 9),
      ],
      [
        call("Shop.Orders", "create_order", 2, "test creates an order/0"),
        call("Shop.Orders", "cancel_order", 1, "test cancels an order/0"),
      ],
    ),
    ["tests"],
  ],
  [
    extraFile(
      "pricing_test",
      "test/shop/pricing_test.exs",
      "Shop.PricingTest",
      [fn("test applies a discount", 0, "modified", 3, 1)],
      [call("Shop.Pricing", "apply_discount", 2, "test applies a discount/0")],
    ),
    ["tests"],
  ],
  [
    extraFile(
      "application",
      "lib/shop/application.ex",
      "Shop.Application",
      [fn("start", 2, "modified", 4, 1)],
      [],
      40,
    ),
    [UNGROUPED_ID],
  ],
  [
    extraFile(
      "seed_task",
      "lib/mix/tasks/shop.seed.ex",
      "Mix.Tasks.Shop.Seed",
      [fn("run", 1, "added", 19, 0)],
    ),
    [UNGROUPED_ID],
  ],
];

/** Twelve changed files over five groups and the default group. */
export const GROUPED_FILES: GroupedFile[] = FILES_WITH_GROUPS.map(([file, group_ids]) => ({
  ...file,
  group_ids,
}));

/* -- Groups --------------------------------------------------------------------- */

export function findGroup(groups: GroupSummary[], id: string): GroupSummary {
  const group = groups.find((candidate) => candidate.id === id);
  if (!group) throw new Error(`Missing example group ${id}`);
  return group;
}

export function filesInGroup(files: GroupedFile[], groupId: string): GroupedFile[] {
  return files.filter((file) => file.group_ids.includes(groupId));
}

/* -- Summaries ------------------------------------------------------------------ */

export interface Stats {
  file_count: number;
  function_count: number;
  function_counts: Record<ChangeKind, number>;
  added_lines: number;
  removed_lines: number;
  /** Callers still calling something removed or renamed. */
  stale_caller_count: number;
  /** Lowest coverage of any file. Missing when no file has coverage. */
  lowest_coverage?: number;
}

export function summarizeFiles(files: ExampleFile[]): Stats {
  const stats: Stats = {
    file_count: files.length,
    function_count: 0,
    function_counts: { added: 0, modified: 0, removed: 0, renamed: 0 },
    added_lines: 0,
    removed_lines: 0,
    stale_caller_count: 0,
  };
  for (const file of files) {
    for (const changed of file.functions) {
      stats.function_count += 1;
      stats.function_counts[changed.kind] += 1;
      stats.added_lines += changed.added_lines ?? 0;
      stats.removed_lines += changed.removed_lines ?? 0;
    }
    stats.stale_caller_count += file.callers.filter((caller) => caller.stale).length;
    if (file.coverage_percent !== undefined) {
      stats.lowest_coverage = Math.min(stats.lowest_coverage ?? 100, file.coverage_percent);
    }
  }
  return stats;
}

/**
 * Which groups hold a file path. Stand-in for the pattern matching Rust does for any
 * path, changed or not. Dependencies and the standard library are in no group.
 */
export function groupIdsForPath(files: GroupedFile[], path: string): string[] {
  const changed = files.find((file) => file.path === path);
  if (changed) return changed.group_ids;
  if (path.startsWith("deps/") || path.startsWith("lib/elixir/") || path.startsWith("lib/logger/")) {
    return [];
  }
  if (path.startsWith("lib/shop/repo/") || path.startsWith("lib/shop/schemas/")) return ["data"];
  if (/^lib\/shop\/[^/]+\.ex$/.test(path)) return ["business"];
  if (path.startsWith("lib/shop_web/controllers/")) return ["controllers"];
  if (path.startsWith("lib/shop_web/live/")) return ["live"];
  if (path.startsWith("test/")) return ["tests"];
  return [UNGROUPED_ID];
}
