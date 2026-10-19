import type { FileDiffTarget, FunctionDiffTarget } from "../api/types";

/*
 * Hardcoded example for the design explorations. Nothing here comes from the
 * backend. The shape mirrors what `analyzeChanges` returns once its function
 * changes are grouped by file, so the winning design can be wired up later.
 */

export type ChangeKind = "added" | "modified" | "removed" | "renamed";
export type CallOrigin = "project" | "dependency" | "standard_library";
export type CallerKind = "production" | "test";
/** How a call compares to the base revision. */
export type CallDiff = "same" | "new" | "removed";

export interface ChangedFunction {
  name: string;
  arity: number;
  kind: ChangeKind;
  /** Missing when the backend cannot count them. */
  added_lines?: number;
  removed_lines?: number;
  /** Set when renamed, as `name/arity`. */
  previous_name?: string;
  /** Set for real backend data, to load the function's code. */
  diff_target?: FunctionDiffTarget;
}

/** A function in another module that calls into this file. */
export interface ExampleCaller {
  module: string;
  name: string;
  arity: number;
  file_path: string;
  line: number;
  kind: CallerKind;
  /** The changed function it calls, as `name/arity`. */
  calls: string;
  /** Still calls a function that was removed or renamed. */
  stale: boolean;
  /** The caller is itself changed in this review. */
  also_changed: boolean;
}

/** A function in another module that this file calls. */
export interface ExampleCallee {
  module: string;
  name: string;
  arity: number;
  origin: CallOrigin;
  /** File that defines the called function. Missing when unknown. */
  path?: string;
  /** The changed function that makes the call, as `name/arity`. */
  from: string;
  call_count: number;
  diff: CallDiff;
}

export interface ExampleFile {
  id: string;
  path: string;
  module: string;
  /** Missing when there is no test coverage data. */
  coverage_percent?: number;
  /** Set by the backend. Otherwise worked out from the functions. */
  status?: ChangeKind;
  /** A file that was not parsed: no functions, callers or calls, only its text. */
  plain?: boolean;
  /** Set for real backend data, to load the file's text. */
  file_diff_target?: FileDiffTarget;
  functions: ChangedFunction[];
  callers: ExampleCaller[];
  callees: ExampleCallee[];
}

export function functionKey(fn: { name: string; arity: number }): string {
  return `${fn.name}/${fn.arity}`;
}

export function callerLabel(caller: ExampleCaller): string {
  return `${caller.module}.${functionKey(caller)}`;
}

export function calleeLabel(callee: ExampleCallee): string {
  return `${callee.module}.${functionKey(callee)}`;
}

export function shortPath(path: string): string {
  return path.split("/").slice(-2).join("/");
}

function caller(
  module: string,
  name: string,
  arity: number,
  file_path: string,
  line: number,
  calls: string,
  options: Partial<
    Pick<ExampleCaller, "kind" | "stale" | "also_changed">
  > = {},
): ExampleCaller {
  return {
    module,
    name,
    arity,
    file_path,
    line,
    calls,
    kind: options.kind ?? "production",
    stale: options.stale ?? false,
    also_changed: options.also_changed ?? false,
  };
}

function callee(
  module: string,
  name: string,
  arity: number,
  origin: CallOrigin,
  from: string,
  call_count = 1,
  diff: CallDiff = "same",
): ExampleCallee {
  return { module, name, arity, origin, from, call_count, diff };
}

const UNORDERED_FILES: ExampleFile[] = [
  {
    id: "controller",
    path: "lib/shop_web/controllers/order_controller.ex",
    module: "ShopWeb.OrderController",
    coverage_percent: 91,
    functions: [
      { name: "create", arity: 2, kind: "modified", added_lines: 9, removed_lines: 4 },
      { name: "show", arity: 2, kind: "modified", added_lines: 2, removed_lines: 1 },
    ],
    callers: [
      caller(
        "ShopWeb.OrderControllerTest",
        "test POST /orders",
        0,
        "test/shop_web/controllers/order_controller_test.exs",
        14,
        "create/2",
        { kind: "test" },
      ),
    ],
    callees: [
      callee("Shop.Orders", "create_order", 2, "project", "create/2", 1, "same"),
      callee("Shop.Orders", "get_order!", 1, "project", "show/2"),
      callee("Shop.Accounts", "get_user!", 1, "project", "create/2", 1, "new"),
      callee("Plug.Conn", "put_status", 2, "dependency", "create/2"),
      callee("Phoenix.Controller", "json", 2, "dependency", "create/2", 2),
      callee("Phoenix.Controller", "json", 2, "dependency", "show/2"),
    ],
  },
  {
    id: "orders",
    path: "lib/shop/orders.ex",
    module: "Shop.Orders",
    coverage_percent: 72,
    functions: [
      { name: "create_order", arity: 2, kind: "modified", added_lines: 18, removed_lines: 6 },
      { name: "cancel_order", arity: 1, kind: "added", added_lines: 24, removed_lines: 0 },
      { name: "legacy_total", arity: 1, kind: "removed", added_lines: 0, removed_lines: 9 },
    ],
    callers: [
      caller(
        "ShopWeb.OrderController",
        "create",
        2,
        "lib/shop_web/controllers/order_controller.ex",
        14,
        "create_order/2",
        { also_changed: true },
      ),
      caller(
        "ShopWeb.OrderLive",
        "handle_event",
        3,
        "lib/shop_web/live/order_live.ex",
        41,
        "create_order/2",
      ),
      caller(
        "ShopWeb.OrderLive",
        "handle_event",
        3,
        "lib/shop_web/live/order_live.ex",
        58,
        "cancel_order/1",
      ),
      caller("Shop.Checkout", "finish", 1, "lib/shop/checkout.ex", 77, "create_order/2"),
      caller("Shop.Cart", "summary", 1, "lib/shop/cart.ex", 30, "legacy_total/1", {
        stale: true,
      }),
      caller(
        "Shop.OrdersTest",
        "test creates an order",
        0,
        "test/shop/orders_test.exs",
        12,
        "create_order/2",
        { kind: "test" },
      ),
      caller(
        "Shop.OrdersTest",
        "test cancels an order",
        0,
        "test/shop/orders_test.exs",
        40,
        "cancel_order/1",
        { kind: "test" },
      ),
    ],
    callees: [
      callee("Shop.Pricing", "subtotal", 1, "project", "create_order/2", 1, "new"),
      callee("Shop.Inventory", "reserve", 2, "project", "create_order/2", 1, "new"),
      callee("Shop.Inventory", "release", 2, "project", "cancel_order/1", 1, "new"),
      callee("Shop.Repo", "insert", 1, "project", "create_order/2", 2),
      callee("Shop.Repo", "update", 1, "project", "cancel_order/1", 1, "new"),
      callee("Shop.Mailer", "deliver_confirmation", 1, "project", "create_order/2"),
      callee("Ecto.Multi", "new", 0, "dependency", "create_order/2"),
      callee("Ecto.Multi", "insert", 3, "dependency", "create_order/2", 3),
      callee("Phoenix.PubSub", "broadcast", 3, "dependency", "cancel_order/1", 1, "new"),
      callee("Logger", "info", 1, "standard_library", "create_order/2", 2),
      callee("Enum", "map", 2, "standard_library", "legacy_total/1", 1, "removed"),
    ],
  },
  {
    id: "inventory",
    path: "lib/shop/inventory.ex",
    module: "Shop.Inventory",
    coverage_percent: 58,
    functions: [
      { name: "reserve", arity: 2, kind: "modified", added_lines: 12, removed_lines: 3 },
      { name: "release", arity: 2, kind: "added", added_lines: 15, removed_lines: 0 },
    ],
    callers: [
      caller("Shop.Orders", "create_order", 2, "lib/shop/orders.ex", 32, "reserve/2", {
        also_changed: true,
      }),
      caller("Shop.Orders", "cancel_order", 1, "lib/shop/orders.ex", 61, "release/2", {
        also_changed: true,
      }),
      caller(
        "ShopWeb.Admin.InventoryLive",
        "handle_event",
        3,
        "lib/shop_web/live/admin/inventory_live.ex",
        22,
        "reserve/2",
      ),
      caller(
        "Shop.InventoryTest",
        "test reserves stock",
        0,
        "test/shop/inventory_test.exs",
        9,
        "reserve/2",
        { kind: "test" },
      ),
    ],
    callees: [
      callee("Shop.Repo", "update_all", 2, "project", "reserve/2", 2),
      callee("Shop.Repo", "transaction", 1, "project", "release/2", 1, "new"),
      callee("Phoenix.PubSub", "broadcast", 3, "dependency", "release/2", 1, "new"),
      callee("Map", "fetch!", 2, "standard_library", "reserve/2"),
      callee("Logger", "warning", 1, "standard_library", "reserve/2"),
    ],
  },
  {
    id: "pricing",
    path: "lib/shop/pricing.ex",
    module: "Shop.Pricing",
    coverage_percent: 84,
    functions: [
      {
        name: "subtotal",
        arity: 1,
        kind: "renamed",
        previous_name: "total/1",
        added_lines: 3,
        removed_lines: 3,
      },
      { name: "apply_discount", arity: 2, kind: "modified", added_lines: 7, removed_lines: 2 },
    ],
    callers: [
      caller("Shop.Orders", "create_order", 2, "lib/shop/orders.ex", 29, "subtotal/1", {
        also_changed: true,
      }),
      caller("Shop.Cart", "summary", 1, "lib/shop/cart.ex", 33, "subtotal/1", {
        stale: true,
      }),
      caller("Shop.Checkout", "finish", 1, "lib/shop/checkout.ex", 70, "apply_discount/2"),
      caller(
        "ShopWeb.CartLive",
        "render",
        1,
        "lib/shop_web/live/cart_live.ex",
        12,
        "apply_discount/2",
      ),
      caller(
        "Shop.PricingTest",
        "test applies a discount",
        0,
        "test/shop/pricing_test.exs",
        8,
        "apply_discount/2",
        { kind: "test" },
      ),
    ],
    callees: [
      callee("Shop.Promotions", "active_for", 1, "project", "apply_discount/2", 1, "new"),
      callee("Decimal", "mult", 2, "dependency", "apply_discount/2", 2),
      callee("Decimal", "add", 2, "dependency", "subtotal/1"),
      callee("Enum", "reduce", 3, "standard_library", "subtotal/1"),
    ],
  },
];

function functionIndex(file: ExampleFile, key: string): number {
  const index = file.functions.findIndex((fn) => functionKey(fn) === key);
  return index === -1 ? file.functions.length : index;
}

function callerRank(caller: ExampleCaller): number {
  if (caller.stale) return 0;
  return caller.kind === "test" ? 2 : 1;
}

/**
 * Callers grouped by the function they call, in the file's function order.
 * Inside a function: stale first, production next, tests last.
 */
export function orderCallers(file: ExampleFile): ExampleCaller[] {
  return [...file.callers].sort(
    (a, b) =>
      functionIndex(file, a.calls) - functionIndex(file, b.calls) ||
      callerRank(a) - callerRank(b),
  );
}

/**
 * Calls grouped by the function that makes them, in the file's function
 * order. Inside a function: calls into the project first.
 */
export function orderCallees(file: ExampleFile): ExampleCallee[] {
  const originRank = (callee: ExampleCallee) => (callee.origin === "project" ? 0 : 1);
  return [...file.callees].sort(
    (a, b) =>
      functionIndex(file, a.from) - functionIndex(file, b.from) || originRank(a) - originRank(b),
  );
}

/** All functions added is added, all removed is removed, anything else is modified. */
export function fileStatus(file: ExampleFile): ChangeKind {
  if (file.status) return file.status;
  const kinds = new Set(file.functions.map((fn) => fn.kind));
  if (kinds.size === 1 && (kinds.has("added") || kinds.has("removed"))) {
    return file.functions[0]?.kind ?? "modified";
  }
  return "modified";
}

export interface FileLink {
  from_file_id: string;
  to_file_id: string;
  /** Functions in the receiving file, as `name/arity`. */
  functions: string[];
  call_count: number;
}

/** Calls from one changed file into another changed file. */
export function findFileLinks(files: ExampleFile[]): FileLink[] {
  const links = new Map<string, FileLink>();
  for (const from of files) {
    for (const call of from.callees) {
      const to = files.find(
        (candidate) =>
          candidate.id !== from.id &&
          candidate.module === call.module &&
          candidate.functions.some((fn) => functionKey(fn) === functionKey(call)),
      );
      if (!to) continue;
      const key = `${from.id}->${to.id}`;
      const link = links.get(key) ?? {
        from_file_id: from.id,
        to_file_id: to.id,
        functions: [],
        call_count: 0,
      };
      const name = functionKey(call);
      if (!link.functions.includes(name)) link.functions.push(name);
      link.call_count += call.call_count;
      links.set(key, link);
    }
  }
  return [...links.values()];
}

export function findFileForCaller(
  files: ExampleFile[],
  caller: ExampleCaller,
): ExampleFile | undefined {
  return files.find((file) => file.module === caller.module);
}

export function findFileForCallee(
  files: ExampleFile[],
  callee: ExampleCallee,
): ExampleFile | undefined {
  return files.find(
    (file) =>
      file.module === callee.module &&
      file.functions.some((fn) => functionKey(fn) === functionKey(callee)),
  );
}

export function groupBy<T>(items: T[], keyOf: (item: T) => string): Map<string, T[]> {
  const groups = new Map<string, T[]>();
  for (const item of items) {
    const key = keyOf(item);
    groups.set(key, [...(groups.get(key) ?? []), item]);
  }
  return groups;
}

/* -- Dependencies and the standard library ---------------------------------- */

/** How calls that rarely need review are drawn. */
export type OutsideMode = "dimmed" | "folded" | "hidden";

export const OUTSIDE_MODE_OPTIONS: { value: OutsideMode; label: string }[] = [
  { value: "dimmed", label: "Dimmed" },
  { value: "folded", label: "Folded" },
  { value: "hidden", label: "Hidden" },
];

/** Dependencies and the standard library. */
export function isOutsideProject(callee: ExampleCallee): boolean {
  return callee.origin !== "project";
}

/** Calls into the project, with outside calls according to the mode. */
export function splitCallees(
  callees: ExampleCallee[],
  mode: OutsideMode,
): { shown: ExampleCallee[]; folded: ExampleCallee[] } {
  const outside = callees.filter(isOutsideProject);
  const project = callees.filter((callee) => !isOutsideProject(callee));
  if (mode === "dimmed") return { shown: callees, folded: [] };
  return { shown: project, folded: mode === "folded" ? outside : [] };
}

/* -- Where a called module lives ---------------------------------------------- */

const MODULE_PATHS: Record<string, string> = {
  "Shop.Accounts": "lib/shop/accounts.ex",
  "Shop.Inventory": "lib/shop/inventory.ex",
  "Shop.Mailer": "lib/shop/mailer.ex",
  "Shop.Orders": "lib/shop/orders.ex",
  "Shop.Pricing": "lib/shop/pricing.ex",
  "Shop.Promotions": "lib/shop/promotions.ex",
  "Shop.Repo": "lib/shop/repo.ex",
  "Phoenix.Controller": "deps/phoenix/lib/phoenix/controller.ex",
  "Phoenix.PubSub": "deps/phoenix_pubsub/lib/phoenix/pubsub.ex",
  "Plug.Conn": "deps/plug/lib/plug/conn.ex",
  "Ecto.Multi": "deps/ecto/lib/ecto/multi.ex",
  Decimal: "deps/decimal/lib/decimal.ex",
  Enum: "lib/elixir/lib/enum.ex",
  Map: "lib/elixir/lib/map.ex",
  Logger: "lib/logger/lib/logger.ex",
};

/** Hardcoded. The backend will report the file of every called function. */
export function calleePath(callee: ExampleCallee): string {
  return MODULE_PATHS[callee.module] ?? `lib/${callee.module.toLowerCase().replace(/\./g, "/")}.ex`;
}

export const EXAMPLE_FILES: ExampleFile[] = UNORDERED_FILES.map((file) => ({
  ...file,
  callers: orderCallers(file),
  callees: orderCallees(file).map((callee) => ({ ...callee, path: calleePath(callee) })),
}));
