import { useEffect, useMemo, useRef, useState, type ReactNode } from "react";
import { ChevronRightIcon, DocumentTextIcon } from "@heroicons/react/16/solid";
import type { GroupSummary } from "../../api/types";
import { SegmentedControl, Toolbar, toggled } from "../../explorations/controls";
import { buildGroupRow, type GroupLine, type GroupLink, type GroupRow } from "../../explorations/group_rows";
import {
  FunctionsView,
  GroupRowView,
  GroupRows,
  RENDERER_OPTIONS,
  type GroupRowHandlers,
  type Renderer,
} from "../../explorations/group_views";
import {
  EXAMPLE_GROUPS,
  GROUPED_FILES,
  UNGROUPED_ID,
  childGroups,
  filesInGroup,
  findGroup,
  groupPath,
} from "../../explorations/groups_data";
import { StatsLine, groupIcon } from "../../explorations/groups_parts";
import { plural } from "../../explorations/nodes";
import { ExplorationPage, Option, Section } from "../../explorations/scaffold";
import {
  ScrollTreeLayout,
  anchorId,
  scrollToElement,
  type TreeItem,
} from "../../explorations/scroll_tree";

const FILES = GROUPED_FILES;
const GROUPS = EXAMPLE_GROUPS;
const TOP_LEVEL = childGroups(GROUPS, null);

function rowFor(groupId: string): GroupRow {
  return buildGroupRow(FILES, GROUPS, groupId);
}

function fileById(id: string) {
  const file = FILES.find((candidate) => candidate.id === id);
  if (!file) throw new Error(`Missing example file ${id}`);
  return file;
}

/** Static rows ignore clicks. */
const NO_HANDLERS: GroupRowHandlers = { onOpenLine: () => {}, onOpenLink: () => {} };

/* -- The two levels, side by side ------------------------------------------------- */

function MappingTable() {
  const rows: [string, string, string][] = [
    ["Header", "A file (module and path)", "A group (label and id)"],
    ["Content column", "File: the changed functions", "Files: the changed files. Subgroups for a parent group."],
    ["Outputs", "Functions in other modules that it calls", "Files outside the group that its files call"],
    ["Inputs", "Callers in other modules", "Files outside the group that call its files"],
    ["Click a content line", "Opens a call trace (later)", "A file opens the Functions view. A subgroup opens its own Groups view."],
  ];
  return (
    <table className="w-full border-collapse overflow-hidden rounded-xl border border-base-300 bg-base-100 text-left text-xs">
      <thead>
        <tr className="border-b border-base-300 text-[10px] uppercase tracking-wide text-base-content/50">
          <th className="px-4 py-2 font-medium" />
          <th className="px-4 py-2 font-medium">Functions view (today)</th>
          <th className="px-4 py-2 font-medium">Groups view (new)</th>
        </tr>
      </thead>
      <tbody>
        {rows.map(([label, functions, groups]) => (
          <tr key={label} className="border-b border-base-300 last:border-b-0">
            <th className="px-4 py-2 font-medium">{label}</th>
            <td className="px-4 py-2 text-base-content/70">{functions}</td>
            <td className="px-4 py-2">{groups}</td>
          </tr>
        ))}
      </tbody>
    </table>
  );
}

function OneLevelUp({ renderer }: { renderer: Renderer }) {
  return (
    <div>
      <p className="border-b border-base-300 bg-base-100 px-4 py-1.5 text-[11px] font-medium text-base-content/60">
        Functions view: one file
      </p>
      <FunctionsView files={[fileById("orders")]} renderer={renderer} />
      <p className="border-y border-base-300 bg-base-100 px-4 py-1.5 text-[11px] font-medium text-base-content/60">
        Groups view: the group that holds it, Core / Business logic
      </p>
      <GroupRows rows={[rowFor("core/business")]} renderer={renderer} handlers={NO_HANDLERS} />
    </div>
  );
}

function ParentRow({ renderer }: { renderer: Renderer }) {
  return (
    <GroupRows
      rows={[rowFor("core"), rowFor("web")]}
      renderer={renderer}
      handlers={NO_HANDLERS}
    />
  );
}

/* -- Flow state ---------------------------------------------------------------------- */

/**
 * Where the reader is. The Groups view shows every top-level group, or one group alone.
 * The Functions view shows every file of one group.
 */
type Place =
  | { level: "groups"; groupId: string | null }
  | { level: "functions"; groupId: string; fileId: string | null };

const ALL_GROUPS: Place = { level: "groups", groupId: null };

function useFlow() {
  const [place, setPlace] = useState<Place>(ALL_GROUPS);
  const handlers: GroupRowHandlers = {
    onOpenLine: (row: GroupRow, line: GroupLine) =>
      setPlace(
        line.kind === "subgroup"
          ? { level: "groups", groupId: line.id }
          : { level: "functions", groupId: row.group.id, fileId: line.id },
      ),
    onOpenLink: (link: GroupLink) => setPlace({ level: "groups", groupId: link.group_id }),
  };
  return { place, setPlace, handlers };
}

function Crumb({
  children,
  onClick,
  current = false,
}: {
  children: ReactNode;
  onClick: () => void;
  current?: boolean;
}) {
  return (
    <button
      type="button"
      onClick={onClick}
      className={current ? "font-semibold" : "text-primary hover:underline"}
    >
      {children}
    </button>
  );
}

function Separator() {
  return <ChevronRightIcon className="size-3 text-base-content/40" />;
}

function Breadcrumb({ place, onPlace }: { place: Place; onPlace: (place: Place) => void }) {
  const groupId = place.groupId;
  const trail = groupId ? groupPath(GROUPS, groupId) : [];
  const siblings = groupId ? childGroups(GROUPS, findGroup(GROUPS, groupId).parent_id) : [];
  const inFunctions = place.level === "functions";
  return (
    <div className="space-y-2 border-b border-base-300 bg-base-100 px-4 py-2.5">
      <nav className="flex flex-wrap items-center gap-1.5 text-xs">
        <Crumb onClick={() => onPlace(ALL_GROUPS)} current={trail.length === 0}>
          All changes
        </Crumb>
        {trail.map((group, index) => (
          <span key={group.id} className="flex items-center gap-1.5">
            <Separator />
            <Crumb
              onClick={() => onPlace({ level: "groups", groupId: group.id })}
              current={index === trail.length - 1 && !inFunctions}
            >
              {group.label}
            </Crumb>
          </span>
        ))}
        {inFunctions && (
          <span className="flex items-center gap-1.5">
            <Separator />
            <span className="font-semibold">Functions</span>
          </span>
        )}
      </nav>
      {siblings.length > 1 && (
        <div className="flex flex-wrap gap-1.5">
          {siblings.map((sibling) => (
            <button
              key={sibling.id}
              type="button"
              onClick={() => onPlace({ level: "groups", groupId: sibling.id })}
              className={[
                "rounded-full border px-2.5 py-0.5 text-[11px]",
                sibling.id === groupId
                  ? "border-primary bg-primary/15 font-semibold"
                  : "border-base-300 text-base-content/70 hover:border-base-content/40",
              ].join(" ")}
            >
              {sibling.label}{" "}
              <span className="font-mono text-[10px] text-base-content/50">
                {filesInGroup(FILES, sibling.id).length}
              </span>
            </button>
          ))}
        </div>
      )}
    </div>
  );
}

/** What the place points at: group rows, or the functions of a group's files. */
function FlowBody({
  place,
  renderer,
  handlers,
  onPlace,
}: {
  place: Place;
  renderer: Renderer;
  handlers: GroupRowHandlers;
  onPlace: (place: Place) => void;
}) {
  if (place.level === "groups") {
    const ids = place.groupId === null ? TOP_LEVEL.map((group) => group.id) : [place.groupId];
    return <GroupRows rows={ids.map(rowFor)} renderer={renderer} handlers={handlers} />;
  }
  const group = findGroup(GROUPS, place.groupId);
  const files = filesInGroup(FILES, group.id);
  return (
    <div>
      <div className="flex flex-wrap items-center gap-x-4 gap-y-1 border-b border-base-300 bg-base-100 px-4 py-2 text-[11px]">
        <span className="font-semibold">{group.label}</span>
        <span className="text-base-content/60">{plural(files.length, "file")}</span>
        <StatsLine stats={rowFor(group.id).stats} showFiles={false} />
        <button
          type="button"
          onClick={() => onPlace({ level: "groups", groupId: group.id })}
          className="ml-auto text-primary hover:underline"
        >
          ← Back to the group
        </button>
      </div>
      <FunctionsView files={files} renderer={renderer} focusFileId={place.fileId} />
    </div>
  );
}

/* -- F1: breadcrumb ------------------------------------------------------------------- */

function BreadcrumbFlow({ initialRenderer }: { initialRenderer: Renderer }) {
  const { place, setPlace, handlers } = useFlow();
  const [renderer, setRenderer] = useState<Renderer>(initialRenderer);
  const [grouped, setGrouped] = useState<"defined" | "none">("defined");
  return (
    <div>
      <Toolbar>
        <SegmentedControl label="View" value={renderer} onChange={setRenderer} options={RENDERER_OPTIONS} />
        <SegmentedControl
          label="Groups"
          value={grouped}
          onChange={setGrouped}
          options={[
            { value: "defined", label: "Defined" },
            { value: "none", label: "None defined" },
          ]}
        />
        <span className="text-[11px] text-base-content/50">
          With none, the review opens on the Functions view of every file, as today.
        </span>
      </Toolbar>
      {grouped === "none" ? (
        <FunctionsView files={FILES} renderer={renderer} />
      ) : (
        <>
          <Breadcrumb place={place} onPlace={setPlace} />
          <FlowBody place={place} renderer={renderer} handlers={handlers} onPlace={setPlace} />
        </>
      )}
    </div>
  );
}

/* -- F2a: tree with groups that have files ------------------------------------------------------ */

function depthOf(group: GroupSummary): number {
  return group.parent_id === null ? 0 : 1 + depthOf(findGroup(GROUPS, group.parent_id));
}

const F2A_PREFIX = "f2a-";

type TreeMode = { level: "groups" } | { level: "functions"; groupId: string };

function firstLeafId(groupId: string): string {
  const group = findGroup(GROUPS, groupId);
  if (!group.is_parent) return group.id;
  const [first] = childGroups(GROUPS, groupId);
  return first ? firstLeafId(first.id) : group.id;
}

function rootIdOf(groupId: string): string {
  return groupPath(GROUPS, groupId)[0]?.id ?? groupId;
}

/** Groups with files of their own, in tree order. */
const LEAF_GROUPS = GROUPS.filter((group) => !group.is_parent);

function treeFileItem(group: GroupSummary, file: { id: string; path: string }): TreeItem {
  return {
    id: `${FILE_ITEM_PREFIX}${group.id}:${file.id}`,
    label: file.path.split("/").pop() ?? file.path,
    depth: depthOf(group) + 1,
    icon: <DocumentTextIcon className="size-3.5" />,
    mono: true,
  };
}

/** The top-level group, its subgroups, and the files of every subgroup under it. */
function itemsForRoot(rootId: string): TreeItem[] {
  return GROUPS.filter((group) => rootIdOf(group.id) === rootId).flatMap((group) => {
    const files = group.is_parent
      ? []
      : filesInGroup(FILES, group.id).map((file) => treeFileItem(group, file));
    return [groupTreeItem(group), ...files];
  });
}

function resolveF2aItem(item: TreeItem): Element | null {
  const functions = document.getElementById(anchorId(F2A_PREFIX, "functions"));
  if (item.id.startsWith(FILE_ITEM_PREFIX)) {
    const fileId = item.id.slice(FILE_ITEM_PREFIX.length).split(":")[1];
    return functions?.querySelector(`#review-file-${fileId}`) ?? null;
  }
  const row = document.getElementById(anchorId(F2A_PREFIX, item.id));
  if (row) return row;
  // In the Functions view a group stands for its first file.
  const first = filesInGroup(FILES, item.id)[0];
  return first ? (functions?.querySelector(`#review-file-${first.id}`) ?? null) : null;
}

/** A parent group has no row, so a click on it goes to its first subgroup. */
function scrollTargetF2a(item: TreeItem): Element | null {
  if (item.id.startsWith(FILE_ITEM_PREFIX)) return resolveF2aItem(item);
  return resolveF2aItem({ ...item, id: firstLeafId(item.id) });
}

/**
 * Rows only for groups that have files, in a scrolling page, with every group in the tree.
 * Clicking a file opens the Functions view of its top-level group, and the tree then lists
 * that group, its subgroups and all their files.
 */
function GroupsTreeFlow() {
  const [renderer, setRenderer] = useState<Renderer>("ledger");
  const [mode, setMode] = useState<TreeMode>({ level: "groups" });
  const afterRender = useRef<(() => void) | null>(null);

  useEffect(() => {
    const action = afterRender.current;
    afterRender.current = null;
    if (action) window.setTimeout(action, 80);
  }, [mode]);

  const scrollToItem = (id: string) => scrollToElement(scrollTargetF2a({ id, label: "", depth: 0 }));

  const openFunctions = (groupId: string, fileId: string | null) => {
    afterRender.current = () =>
      scrollToItem(fileId ? `${FILE_ITEM_PREFIX}${groupId}:${fileId}` : groupId);
    setMode({ level: "functions", groupId });
  };
  const backToGroups = (groupId: string) => {
    afterRender.current = () => scrollToItem(groupId);
    setMode({ level: "groups" });
  };

  const handlers: GroupRowHandlers = {
    onOpenLine: (row, line) => {
      if (line.kind === "file") openFunctions(row.group.id, line.id);
    },
    onOpenLink: (link) => scrollToItem(link.group_id),
  };

  const rootId = mode.level === "functions" ? rootIdOf(mode.groupId) : null;
  const items = useMemo(() => (rootId ? itemsForRoot(rootId) : GROUPS.map(groupTreeItem)), [rootId]);

  const heading = (
    <div className="mb-1 flex items-center justify-between gap-2 px-3 pb-2 text-[10px] uppercase tracking-wide text-base-content/50">
      <span>{mode.level === "functions" ? "Functions view" : "Groups view"}</span>
      {mode.level === "functions" && (
        <button
          type="button"
          onClick={() => backToGroups(mode.groupId)}
          className="normal-case text-primary hover:underline"
        >
          ← Back to group view
        </button>
      )}
    </div>
  );

  return (
    <div>
      <Toolbar>
        <SegmentedControl label="View" value={renderer} onChange={setRenderer} options={RENDERER_OPTIONS} />
        <span className="text-[11px] text-base-content/50">
          Parent groups are in the tree but have no row. Click a file line to open the Functions view.
        </span>
      </Toolbar>
      <ScrollTreeLayout
        key={mode.level}
        prefix={F2A_PREFIX}
        items={items}
        resolve={resolveF2aItem}
        scrollTarget={scrollTargetF2a}
        watchKey={`${renderer}:${rootId ?? ""}`}
        heading={heading}
      >
        {mode.level === "groups" ? (
          <GroupRows
            rows={LEAF_GROUPS.map((group) => rowFor(group.id))}
            renderer={renderer}
            handlers={handlers}
            anchor={(row) => anchorId(F2A_PREFIX, row.group.id)}
          />
        ) : (
          <TreeFunctions
            groupId={mode.groupId}
            renderer={renderer}
            onBack={() => backToGroups(mode.groupId)}
          />
        )}
      </ScrollTreeLayout>
    </div>
  );
}

/** The Functions of every subgroup under the top-level group, one after the other. */
function TreeFunctions({
  groupId,
  renderer,
  onBack,
}: {
  groupId: string;
  renderer: Renderer;
  onBack: () => void;
}) {
  const root = findGroup(GROUPS, rootIdOf(groupId));
  const leaves = LEAF_GROUPS.filter((group) => rootIdOf(group.id) === root.id);
  const files = leaves
    .flatMap((leaf) => filesInGroup(FILES, leaf.id))
    .filter((file, index, all) => all.findIndex((other) => other.id === file.id) === index);
  return (
    <div>
      <div
        id={root.is_parent ? anchorId(F2A_PREFIX, root.id) : undefined}
        className="flex items-center gap-3 border-b border-base-300 px-4 py-2 text-xs"
      >
        <button type="button" onClick={onBack} className="shrink-0 text-primary hover:underline">
          ← Back to group view
        </button>
        <span className="text-base-content/30">|</span>
        <span className="min-w-0 truncate">
          Viewing files in <span className="font-semibold">{root.label}</span>
        </span>
        <span className="ml-auto shrink-0 font-mono text-[11px] text-base-content/60">
          {plural(rowFor(root.id).stats.file_count, "file")}
        </span>
      </div>
      <div id={anchorId(F2A_PREFIX, "functions")}>
        <FunctionsView files={files} renderer={renderer} />
      </div>
    </div>
  );
}

/* -- F2b: tree that scrolls --------------------------------------------------------------- */

const F2_PREFIX = "f2-";

const FILE_ITEM_PREFIX = "file:";

function fileItemId(groupId: string, fileId: string): string {
  return `${FILE_ITEM_PREFIX}${groupId}:${fileId}`;
}

const F2_ITEMS: TreeItem[] = GROUPS.flatMap((group) => {
  const Icon = groupIcon(group);
  const groupItem: TreeItem = {
    id: group.id,
    label: group.label,
    depth: depthOf(group),
    icon: <Icon className="size-3.5" />,
    detail: String(filesInGroup(FILES, group.id).length),
    dim: group.id === UNGROUPED_ID,
  };
  const fileItems: TreeItem[] = group.is_parent
    ? []
    : filesInGroup(FILES, group.id).map((file) => ({
        id: fileItemId(group.id, file.id),
        label: file.path.split("/").pop() ?? file.path,
        depth: depthOf(group) + 1,
        icon: <DocumentTextIcon className="size-3.5" />,
        mono: true,
      }));
  return [groupItem, ...fileItems];
});

/** The element an item scrolls to. A file's Functions view lives inside its group's section. */
function resolveF2Item(item: TreeItem): Element | null {
  if (!item.id.startsWith(FILE_ITEM_PREFIX)) {
    return document.getElementById(anchorId(F2_PREFIX, item.id));
  }
  const [groupId, fileId] = item.id.slice(FILE_ITEM_PREFIX.length).split(":");
  return document
    .getElementById(anchorId(F2_PREFIX, `functions:${groupId}`))
    ?.querySelector(`#review-file-${fileId}`) ?? null;
}

/** Every group's row, then the Functions of its files, on one page. The tree only scrolls. */
function ScrollingTreeFlow() {
  const [renderer, setRenderer] = useState<Renderer>("ledger");
  const jumpTo = (id: string) => scrollToElement(resolveF2Item({ id, label: "", depth: 0 }));
  const handlers: GroupRowHandlers = {
    onOpenLine: (row, line) =>
      jumpTo(line.kind === "subgroup" ? line.id : fileItemId(row.group.id, line.id)),
    onOpenLink: (link) => jumpTo(link.group_id),
  };
  return (
    <div>
      <Toolbar>
        <SegmentedControl label="View" value={renderer} onChange={setRenderer} options={RENDERER_OPTIONS} />
        <span className="text-[11px] text-base-content/50">
          Needs the width the Nodes view uses, so Nodes scrolls sideways here.
        </span>
      </Toolbar>
      <ScrollTreeLayout prefix={F2_PREFIX} items={F2_ITEMS} resolve={resolveF2Item} watchKey={renderer}>
        <div className="space-y-8 p-4">
          {GROUPS.map((group) => {
            const files = filesInGroup(FILES, group.id);
            return (
              <section key={group.id} style={{ marginLeft: depthOf(group) * 16 }} className="space-y-3">
                <div id={anchorId(F2_PREFIX, group.id)} className="overflow-x-auto">
                  <div className="w-max">
                    <GroupRowView row={rowFor(group.id)} renderer={renderer} handlers={handlers} />
                  </div>
                </div>
                {!group.is_parent && files.length > 0 && (
                  <div
                    id={anchorId(F2_PREFIX, `functions:${group.id}`)}
                    className="overflow-hidden rounded-xl border border-base-300"
                  >
                    <p className="border-b border-base-300 bg-base-100 px-4 py-1.5 text-[11px] font-medium text-base-content/60">
                      Functions of {group.label}
                    </p>
                    <FunctionsView files={files} renderer={renderer} />
                  </div>
                )}
              </section>
            );
          })}
        </div>
      </ScrollTreeLayout>
    </div>
  );
}

/* -- F2c: tree that starts with groups -------------------------------------------------- */

const F2C_PREFIX = "f2c-";

type HybridMode =
  | { level: "groups" }
  | { level: "functions"; groupId: string; fileId: string | null };

function groupTreeItem(group: GroupSummary): TreeItem {
  const Icon = groupIcon(group);
  return {
    id: group.id,
    label: group.label,
    depth: depthOf(group),
    icon: <Icon className="size-3.5" />,
    detail: String(filesInGroup(FILES, group.id).length),
    dim: group.id === UNGROUPED_ID,
  };
}

const GROUPS_ONLY_ITEMS: TreeItem[] = GROUPS.map(groupTreeItem);

/** Every group, and under the selected one, its files. */
function itemsWithFilesOf(selectedGroupId: string): TreeItem[] {
  return GROUPS.flatMap((group) => {
    const item = groupTreeItem(group);
    if (group.id !== selectedGroupId) return [item];
    const files = filesInGroup(FILES, group.id).map((file) => ({
      id: `${FILE_ITEM_PREFIX}${file.id}`,
      label: file.path.split("/").pop() ?? file.path,
      depth: depthOf(group) + 1,
      icon: <DocumentTextIcon className="size-3.5" />,
      mono: true,
    }));
    return [item, ...files];
  });
}

function resolveHybridItem(item: TreeItem): Element | null {
  if (item.id.startsWith(FILE_ITEM_PREFIX)) {
    const fileId = item.id.slice(FILE_ITEM_PREFIX.length);
    return (
      document
        .getElementById(anchorId(F2C_PREFIX, "functions"))
        ?.querySelector(`#review-file-${fileId}`) ?? null
    );
  }
  return document.getElementById(anchorId(F2C_PREFIX, item.id));
}

/**
 * Starts as one scrolling page of group rows with a tree of groups only. Clicking a file
 * opens the Functions view of its group, and the tree then lists that group's files too.
 */
function HybridTreeFlow() {
  const [renderer, setRenderer] = useState<Renderer>("ledger");
  const [mode, setMode] = useState<HybridMode>({ level: "groups" });
  const afterRender = useRef<(() => void) | null>(null);

  useEffect(() => {
    const action = afterRender.current;
    afterRender.current = null;
    if (action) window.setTimeout(action, 80);
  }, [mode]);

  const scrollToItem = (id: string) => scrollToElement(resolveHybridItem({ id, label: "", depth: 0 }));

  const openFunctions = (groupId: string, fileId: string | null) => {
    afterRender.current = () => scrollToItem(fileId ? `${FILE_ITEM_PREFIX}${fileId}` : groupId);
    setMode({ level: "functions", groupId, fileId });
  };
  const backToGroups = (groupId: string) => {
    afterRender.current = () => scrollToItem(groupId);
    setMode({ level: "groups" });
  };

  const handlers: GroupRowHandlers = {
    onOpenLine: (row, line) =>
      line.kind === "subgroup" ? scrollToItem(line.id) : openFunctions(row.group.id, line.id),
    onOpenLink: (link) => scrollToItem(link.group_id),
  };

  const inFunctions = mode.level === "functions";
  const selectedGroupId = inFunctions ? mode.groupId : null;
  const items = useMemo(
    () => (selectedGroupId ? itemsWithFilesOf(selectedGroupId) : GROUPS_ONLY_ITEMS),
    [selectedGroupId],
  );

  const heading = (
    <div className="mb-1 flex items-center justify-between gap-2 px-3 pb-2 text-[10px] uppercase tracking-wide text-base-content/50">
      <span>{inFunctions ? "Functions view" : "Groups view"}</span>
      {inFunctions && (
        <button
          type="button"
          onClick={() => backToGroups(mode.groupId)}
          className="normal-case text-primary hover:underline"
        >
          ← All groups
        </button>
      )}
    </div>
  );

  return (
    <div>
      <Toolbar>
        <SegmentedControl label="View" value={renderer} onChange={setRenderer} options={RENDERER_OPTIONS} />
        <span className="text-[11px] text-base-content/50">
          Click a file line to go from the Groups view to the Functions view. The tree then adds that group's files.
        </span>
      </Toolbar>
      <ScrollTreeLayout
        key={inFunctions ? "functions" : "groups"}
        prefix={F2C_PREFIX}
        items={items}
        resolve={resolveHybridItem}
        beforeJump={(item) => {
          if (inFunctions && !item.id.startsWith(FILE_ITEM_PREFIX)) openFunctions(item.id, null);
        }}
        watchKey={`${renderer}:${selectedGroupId ?? ""}`}
        heading={heading}
      >
        {mode.level === "groups" ? (
          <GroupRows
            rows={GROUPS.map((group) => rowFor(group.id))}
            renderer={renderer}
            handlers={handlers}
            anchor={(row) => anchorId(F2C_PREFIX, row.group.id)}
          />
        ) : (
          <HybridFunctions groupId={mode.groupId} renderer={renderer} />
        )}
      </ScrollTreeLayout>
    </div>
  );
}

function HybridFunctions({ groupId, renderer }: { groupId: string; renderer: Renderer }) {
  const group = findGroup(GROUPS, groupId);
  const files = filesInGroup(FILES, groupId);
  return (
    <div>
      <div
        id={anchorId(F2C_PREFIX, groupId)}
        className="flex flex-wrap items-center gap-x-4 gap-y-1 border-b border-base-300 px-4 py-2 text-[11px]"
      >
        <span className="text-xs font-semibold">{group.label}</span>
        <StatsLine stats={rowFor(groupId).stats} />
        {group.is_parent && (
          <span className="text-base-content/50">The files of all its subgroups</span>
        )}
      </div>
      <div id={anchorId(F2C_PREFIX, "functions")}>
        <FunctionsView files={files} renderer={renderer} />
      </div>
    </div>
  );
}

/* -- F3: expand in place ---------------------------------------------------------------- */

type Zoom = "groups" | "functions";

const SUBGROUP_KEYS = GROUPS.filter((group) => group.is_parent).flatMap((group) =>
  childGroups(GROUPS, group.id).map((child) => `sub:${child.id}`),
);
const FUNCTION_KEYS = GROUPS.filter((group) => !group.is_parent).map((group) => `fn:${group.id}`);

function openFor(zoom: Zoom): Set<string> {
  return zoom === "groups" ? new Set() : new Set([...SUBGROUP_KEYS, ...FUNCTION_KEYS]);
}

/** One group row. Its subgroups and its functions open underneath it. */
function GroupBlock({
  groupId,
  renderer,
  open,
  onToggle,
}: {
  groupId: string;
  renderer: Renderer;
  open: Set<string>;
  onToggle: (key: string) => void;
}) {
  const row = rowFor(groupId);
  const handlers: GroupRowHandlers = {
    onOpenLine: (_row, line) => onToggle(line.kind === "subgroup" ? `sub:${line.id}` : `fn:${groupId}`),
    onOpenLink: () => {},
  };
  const openSubgroups = row.group.is_parent
    ? childGroups(GROUPS, groupId).filter((child) => open.has(`sub:${child.id}`))
    : [];
  return (
    <div className="space-y-4">
      <div className="overflow-x-auto">
        <div className="w-max">
          <GroupRowView row={row} renderer={renderer} handlers={handlers} />
        </div>
      </div>
      {open.has(`fn:${groupId}`) && (
        <div className="overflow-hidden rounded-xl border border-base-300">
          <p className="border-b border-base-300 bg-base-100 px-4 py-1.5 text-[11px] font-medium text-base-content/60">
            Functions of {row.group.label}
          </p>
          <FunctionsView files={filesInGroup(FILES, groupId)} renderer={renderer} />
        </div>
      )}
      {openSubgroups.map((child) => (
        <div key={child.id} className="ml-8 border-l-2 border-base-300 pl-4">
          <GroupBlock groupId={child.id} renderer={renderer} open={open} onToggle={onToggle} />
        </div>
      ))}
    </div>
  );
}

function ExpandInPlaceFlow() {
  const [renderer, setRenderer] = useState<Renderer>("ledger");
  const [zoom, setZoom] = useState<Zoom>("groups");
  const [open, setOpen] = useState<Set<string>>(() => openFor("groups"));
  const chooseZoom = (next: Zoom) => {
    setZoom(next);
    setOpen(openFor(next));
  };
  return (
    <div>
      <Toolbar>
        <SegmentedControl label="View" value={renderer} onChange={setRenderer} options={RENDERER_OPTIONS} />
        <SegmentedControl
          label="Zoom everything"
          value={zoom}
          onChange={chooseZoom}
          options={[
            { value: "groups", label: "Groups" },
            { value: "functions", label: "Functions" },
          ]}
        />
        <span className="text-[11px] text-base-content/50">
          Or click a subgroup or a file to open just that one, right under its row.
        </span>
      </Toolbar>
      <div className="space-y-6 p-4">
        {TOP_LEVEL.map((group) => (
          <GroupBlock
            key={group.id}
            groupId={group.id}
            renderer={renderer}
            open={open}
            onToggle={(key) => setOpen((current) => toggled(current, key))}
          />
        ))}
      </div>
    </div>
  );
}

/* -- Page ---------------------------------------------------------------------------------- */

export default function GroupsFlow() {
  const [renderer, setRenderer] = useState<Renderer>("ledger");
  return (
    <ExplorationPage
      title="Groups, flow"
      intro={
        <>
          <p>
            Two levels, not three. The Groups view is the file view one level up: same ledger or
            node row, with a group in the header and its files as the content. Clicking a file
            opens the Functions view, which is the view the app has today, for every file in that
            group. A parent group has subgroups as its content, and clicking one opens the same
            Groups view for that subgroup.
          </p>
          <p>
            Parent groups only roll up their subgroups, so a file line only ever appears under a
            group with patterns of its own. Everything is hardcoded and uses the example from the
            other group pages.
          </p>
        </>
      }
    >
      <Section
        id="groups-view"
        title="The Groups view"
        intro="Built from the same pieces as the Functions view, so the two read alike."
      >
        <MappingTable />
        <Option
          code="V1"
          title="One level up"
          note="Hover a file, subgroup or link to highlight what it connects to, as in the file ledger. Click a linked file that is changed in this review to jump to its group. Dependencies and the standard library are folded, because at file level they are noise."
        >
          <Toolbar>
            <SegmentedControl label="View" value={renderer} onChange={setRenderer} options={RENDERER_OPTIONS} />
          </Toolbar>
          <OneLevelUp renderer={renderer} />
        </Option>
        <Option
          code="V2"
          title="A parent group"
          note="A group with subgroups lists them as its content, each with function counts, line counts and a stale marker. Its outputs and inputs are the files outside the whole parent. There are no include or exclude lines on a parent, because it has none."
        >
          <Toolbar>
            <SegmentedControl label="View" value={renderer} onChange={setRenderer} options={RENDERER_OPTIONS} />
          </Toolbar>
          <ParentRow renderer={renderer} />
        </Option>
      </Section>

      <Section
        id="flows"
        title="Flows"
        intro="Click through: Groups, then a subgroup, then a file for its functions. The same Ledger and Nodes toggle applies to both levels."
      >
        <Option
          code="F1"
          title="Breadcrumb"
          note="One level on screen. Click a subgroup line to open its Groups view, a file line to open the Functions view of the whole group (scrolled to that file), and the breadcrumb or a sibling pill to go back or across. Links to other changed files hop to their group. None defined skips straight to today's view."
        >
          <BreadcrumbFlow initialRenderer="ledger" />
        </Option>
        <Option
          code="F2a"
          title="Tree, groups with files"
          clip={false}
          note="The tree lists every group and subgroup and scrolls the page to the one you click. The page only has rows for groups that have files, so a parent group in the tree scrolls to its first subgroup. Click a file to open the Functions view: the tree then shows that top-level group, its subgroups and all their files, and Back to group (at the top or in the tree) returns to the groups."
        >
          <GroupsTreeFlow />
        </Option>
        <Option
          code="F2b"
          title="Tree that scrolls, everything on one page"
          clip={false}
          note="Like the file tree on a GitHub pull request. Every group's row and the Functions of its files are on one long page, and the tree on the left, with groups and files, jumps to any of them by scrolling. Nothing is hidden, and the tree highlights where you are as you scroll."
        >
          <ScrollingTreeFlow />
        </Option>
        <Option
          code="F2c"
          title="Tree that starts with groups"
          clip={false}
          note="The middle ground. It opens on one scrolling page of group rows, with only groups and subgroups in the tree. Click a file line to go to the Functions view of that group. The tree then adds that group's files under it, and picking another group switches the Functions view to it. The link at the top of the tree returns to the groups."
        >
          <HybridTreeFlow />
        </Option>
        <Option
          code="F3"
          title="Expand in place"
          note="The Groups view is the page. Clicking a subgroup opens its row underneath the parent, indented, and clicking a file opens the Functions view of its group underneath the row. A zoom control opens everything to one level. Never leaves the page, but long."
        >
          <ExpandInPlaceFlow />
        </Option>
      </Section>
    </ExplorationPage>
  );
}
