import type { GroupSummary } from "../api/types";
import {
  EXAMPLE_FILES,
  calleePath,
  findFileLinks,
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

/** The ungrouped group is implicit and only exists next to real groups. */
export const UNGROUPED_ID = "ungrouped";

/** A changed file with the ids of every group that holds it. Parents come before subgroups. */
export type GroupedFile = ExampleFile & { group_ids: string[] };

/** What `list_groups` returns for the example config below. */
export const EXAMPLE_GROUPS: GroupSummary[] = [
  { id: "core", parent_id: null, label: "Core", is_parent: true },
  { id: "core/data", parent_id: "core", label: "Data", is_parent: false },
  { id: "core/business", parent_id: "core", label: "Business logic", is_parent: false },
  { id: "web", parent_id: null, label: "Web", is_parent: true },
  { id: "web/controllers", parent_id: "web", label: "Controllers", is_parent: false },
  { id: "web/live", parent_id: "web", label: "LiveViews", is_parent: false },
  { id: "tests", parent_id: null, label: "Tests", is_parent: false },
  { id: UNGROUPED_ID, parent_id: null, label: "Ungrouped", is_parent: false },
];

/** The `.code_reviewer.toml` the groups above come from. */
export const EXAMPLE_GROUPS_TOML = `# .code_reviewer.toml
[groups.core]
label = "Core"

[groups.core.subgroups.data]
label = "Data"
include = ["lib/shop/schemas/**", "lib/shop/repo/**"]

[groups.core.subgroups.business]
label = "Business logic"
include = ["lib/shop/*.ex"]
exclude = ["lib/shop/application.ex"]

[groups.web]
label = "Web"

[groups.web.subgroups.controllers]
label = "Controllers"
include = ["lib/shop_web/controllers/**"]

[groups.web.subgroups.live]
label = "LiveViews"
include = ["lib/shop_web/live/**"]

[groups.tests]
label = "Tests"
include = ["test/**"]
`;

/** Hand-written stand-in for the pattern matching the backend does. */
export const EXAMPLE_PATTERNS: Record<string, { include: string[]; exclude: string[] }> = {
  "core/data": { include: ["lib/shop/schemas/**", "lib/shop/repo/**"], exclude: [] },
  "core/business": { include: ["lib/shop/*.ex"], exclude: ["lib/shop/application.ex"] },
  "web/controllers": { include: ["lib/shop_web/controllers/**"], exclude: [] },
  "web/live": { include: ["lib/shop_web/live/**"], exclude: [] },
  tests: { include: ["test/**"], exclude: [] },
};

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
  [byId("controller"), ["web", "web/controllers"]],
  [
    withCalls(byId("orders"), [
      call("Shop.Repo.Queries", "open_orders", 0, "create_order/2"),
      call("Shop.Schemas.Order", "changeset", 2, "create_order/2"),
    ]),
    ["core", "core/business"],
  ],
  [byId("inventory"), ["core", "core/business"]],
  [byId("pricing"), ["core", "core/business"]],
  [
    extraFile(
      "schema_order",
      "lib/shop/schemas/order.ex",
      "Shop.Schemas.Order",
      [fn("changeset", 2, "modified", 8, 2), fn("status_values", 0, "added", 6, 0)],
      [],
      88,
    ),
    ["core", "core/data"],
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
    ["core", "core/data"],
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
    ["web", "web/live"],
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
    ["web", "web/live"],
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

/** Twelve changed files over seven groups and "Ungrouped". */
export const GROUPED_FILES: GroupedFile[] = FILES_WITH_GROUPS.map(([file, group_ids]) => ({
  ...file,
  group_ids,
}));

/* -- Tree ----------------------------------------------------------------------- */

export function childGroups(groups: GroupSummary[], parentId: string | null): GroupSummary[] {
  return groups.filter((group) => group.parent_id === parentId);
}

export function findGroup(groups: GroupSummary[], id: string): GroupSummary {
  const group = groups.find((candidate) => candidate.id === id);
  if (!group) throw new Error(`Missing example group ${id}`);
  return group;
}

/** From the top group down to this one. */
export function groupPath(groups: GroupSummary[], id: string): GroupSummary[] {
  const group = findGroup(groups, id);
  return group.parent_id === null ? [group] : [...groupPath(groups, group.parent_id), group];
}

export function filesInGroup(files: GroupedFile[], groupId: string): GroupedFile[] {
  return files.filter((file) => file.group_ids.includes(groupId));
}

/** Where a drill-down level can go next: a parent shows its subgroups, a leaf shows files. */
export function isLeaf(group: GroupSummary): boolean {
  return !group.is_parent;
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

/* -- Calls between groups --------------------------------------------------------- */

export interface GroupLink {
  from_id: string;
  to_id: string;
  call_count: number;
}

/** Calls from files of one group into files of another, among the given groups. */
export function findGroupLinks(files: GroupedFile[], viewGroupIds: string[]): GroupLink[] {
  const links = new Map<string, GroupLink>();
  for (const link of findFileLinks(files)) {
    const from = files.find((file) => file.id === link.from_file_id);
    const to = files.find((file) => file.id === link.to_file_id);
    if (!from || !to) continue;
    for (const fromId of viewGroupIds.filter((id) => from.group_ids.includes(id))) {
      for (const toId of viewGroupIds.filter((id) => to.group_ids.includes(id))) {
        if (fromId === toId) continue;
        const key = `${fromId}->${toId}`;
        const existing = links.get(key) ?? { from_id: fromId, to_id: toId, call_count: 0 };
        existing.call_count += link.call_count;
        links.set(key, existing);
      }
    }
  }
  return [...links.values()];
}

export interface OutsideLinks {
  /** Other top-level groups that call into this group. */
  incoming: { group_id: string; call_count: number }[];
  /** Other top-level groups this group calls. */
  outgoing: { group_id: string; call_count: number }[];
}

/** Calls across the edge of one group, counted by the top-level group on the other side. */
export function findOutsideLinks(
  files: GroupedFile[],
  groups: GroupSummary[],
  groupId: string,
): OutsideLinks {
  const topIds = childGroups(groups, null).map((group) => group.id);
  const incoming = new Map<string, number>();
  const outgoing = new Map<string, number>();
  for (const link of findFileLinks(files)) {
    const from = files.find((file) => file.id === link.from_file_id);
    const to = files.find((file) => file.id === link.to_file_id);
    if (!from || !to) continue;
    const fromInside = from.group_ids.includes(groupId);
    const toInside = to.group_ids.includes(groupId);
    if (fromInside === toInside) continue;
    const [outsideFile, counts] = fromInside ? [to, outgoing] : [from, incoming];
    for (const topId of topIds.filter((id) => outsideFile.group_ids.includes(id))) {
      counts.set(topId, (counts.get(topId) ?? 0) + link.call_count);
    }
    if (outsideFile.group_ids.includes(UNGROUPED_ID)) {
      counts.set(UNGROUPED_ID, (counts.get(UNGROUPED_ID) ?? 0) + link.call_count);
    }
  }
  const toList = (counts: Map<string, number>) =>
    [...counts.entries()].map(([group_id, call_count]) => ({ group_id, call_count }));
  return { incoming: toList(incoming), outgoing: toList(outgoing) };
}

/**
 * Which groups hold a file path, parents before subgroups. Stand-in for the pattern
 * matching Rust does for any path, changed or not. Dependencies and the standard
 * library are in no group.
 */
export function groupIdsForPath(files: GroupedFile[], path: string): string[] {
  const changed = files.find((file) => file.path === path);
  if (changed) return changed.group_ids;
  if (path.startsWith("deps/") || path.startsWith("lib/elixir/") || path.startsWith("lib/logger/")) {
    return [];
  }
  if (path.startsWith("lib/shop/repo/") || path.startsWith("lib/shop/schemas/")) {
    return ["core", "core/data"];
  }
  if (/^lib\/shop\/[^/]+\.ex$/.test(path)) return ["core", "core/business"];
  if (path.startsWith("lib/shop_web/controllers/")) return ["web", "web/controllers"];
  if (path.startsWith("lib/shop_web/live/")) return ["web", "web/live"];
  if (path.startsWith("test/")) return ["tests"];
  return [UNGROUPED_ID];
}
