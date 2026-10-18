import { useEffect, useMemo, useRef, useState, type ReactNode } from "react";
import { DocumentTextIcon } from "@heroicons/react/16/solid";
import type { GroupSummary } from "../api/types";
import { SegmentedControl, Toolbar } from "../explorations/controls";
import type { GroupRow } from "../explorations/group_rows";
import {
  FunctionsView,
  GroupRows,
  RENDERER_OPTIONS,
  type GroupRowHandlers,
  type Renderer,
} from "../explorations/group_views";
import { UNGROUPED_ID, filesInGroup, findGroup, type GroupedFile } from "../explorations/groups_data";
import { StatsLine, groupIcon } from "../explorations/groups_parts";
import {
  ScrollTreeLayout,
  anchorId,
  scrollToElement,
  type TreeItem,
} from "../explorations/scroll_tree";

/*
 * A review whose files are in groups. The tree is All changes by group, with the
 * groups under it, and one click always means the same view:
 * - All changes by group shows only the summary row.
 * - A group shows the Groups view (a row per group), scrolled to that group, and
 *   lists the group's files under it in the tree.
 * - A file, in the tree or in a row, shows the Functions view of its group.
 */

const PREFIX = "grouped-";
const FILE_PREFIX = "file:";

const HEADINGS = {
  summary: "Summary view",
  groups: "Groups view",
  functions: "Functions view",
};

/** Which view is open, and the group whose files the tree lists. */
interface View {
  level: "summary" | "groups" | "functions";
  groupId: string | null;
}

function fileItemId(groupId: string, fileId: string): string {
  return `${FILE_PREFIX}${groupId}:${fileId}`;
}

function parseFileItemId(itemId: string): { groupId: string; fileId: string } {
  const rest = itemId.slice(FILE_PREFIX.length);
  const split = rest.lastIndexOf(":");
  return { groupId: rest.slice(0, split), fileId: rest.slice(split + 1) };
}

function fileLabel(file: GroupedFile): string {
  return file.path.split("/").pop() ?? file.path;
}

function groupItem(files: GroupedFile[], group: GroupSummary, depth: number): TreeItem {
  const Icon = groupIcon(group);
  return {
    id: group.id,
    label: group.label,
    depth,
    icon: <Icon className="size-3.5" />,
    detail: String(filesInGroup(files, group.id).length),
    dim: group.id === UNGROUPED_ID,
  };
}

/** All changes, every group under it, and the selected group's files under that group. */
function listItems(
  allRow: GroupRow,
  groups: GroupSummary[],
  files: GroupedFile[],
  selectedGroupId: string | null,
): TreeItem[] {
  const allItem: TreeItem = {
    ...groupItem(files, allRow.group, 0),
    detail: String(allRow.stats.file_count),
  };
  const groupItems = groups.flatMap((group) => {
    const item = groupItem(files, group, 1);
    if (group.id !== selectedGroupId) return [item];
    const fileItems = filesInGroup(files, group.id).map((file) => ({
      id: fileItemId(group.id, file.id),
      label: fileLabel(file),
      depth: 2,
      icon: <DocumentTextIcon className="size-3.5" />,
      mono: true,
    }));
    return [item, ...fileItems];
  });
  return [allItem, ...groupItems];
}

function resolveItem(item: TreeItem): Element | null {
  if (item.id.startsWith(FILE_PREFIX)) {
    const { fileId } = parseFileItemId(item.id);
    return (
      document
        .getElementById(anchorId(PREFIX, "functions"))
        ?.querySelector(`#review-file-${fileId}`) ?? null
    );
  }
  return document.getElementById(anchorId(PREFIX, item.id));
}

export function GroupedReview({
  groups,
  files,
  allRow,
  rows,
  toolbarEnd,
}: {
  groups: GroupSummary[];
  files: GroupedFile[];
  /** Rolls up every group. The only row of the summary view. */
  allRow: GroupRow;
  rows: GroupRow[];
  /** Shown at the right end of the toolbar. */
  toolbarEnd?: ReactNode;
}) {
  const [renderer, setRenderer] = useState<Renderer>("ledger");
  const [view, setView] = useState<View>({ level: "summary", groupId: null });
  const afterRender = useRef<(() => void) | null>(null);

  useEffect(() => {
    const action = afterRender.current;
    afterRender.current = null;
    if (action) window.setTimeout(action, 80);
  }, [view]);

  const scrollToItem = (id: string) => scrollToElement(resolveItem({ id, label: "", depth: 0 }));

  const openFunctions = (groupId: string, fileId: string | null) => {
    afterRender.current = () => scrollToItem(fileId ? fileItemId(groupId, fileId) : groupId);
    setView({ level: "functions", groupId });
  };
  /** Shows the Groups view, with the group's files in the tree, scrolled to the group. */
  const openGroups = (groupId: string) => {
    afterRender.current = () => scrollToItem(groupId);
    setView({ level: "groups", groupId });
  };
  const openSummary = () => {
    afterRender.current = () => scrollToItem(allRow.group.id);
    setView({ level: "summary", groupId: null });
  };

  const handlers: GroupRowHandlers = {
    onOpenLine: (row, line) =>
      line.kind === "group" ? openGroups(line.id) : openFunctions(row.group.id, line.id),
    onOpenLink: (link) => {
      if (groups.some((group) => group.id === link.group_id)) openGroups(link.group_id);
    },
  };

  const inFunctions = view.level === "functions";
  const items = useMemo(
    () => listItems(allRow, groups, files, view.groupId),
    [allRow, groups, files, view.groupId],
  );

  const openTreeItem = (item: TreeItem) => {
    if (item.id.startsWith(FILE_PREFIX)) {
      const { groupId, fileId } = parseFileItemId(item.id);
      if (!inFunctions) openFunctions(groupId, fileId);
    } else if (item.id === allRow.group.id) {
      openSummary();
    } else if (view.level === "groups") {
      setView({ level: "groups", groupId: item.id });
    } else {
      openGroups(item.id);
    }
  };

  const heading = (
    <div className="mb-1 px-3 pb-2 text-[10px] uppercase tracking-wide text-base-content/50">
      {HEADINGS[view.level]}
    </div>
  );

  return (
    <div>
      <Toolbar>
        <SegmentedControl label="View" value={renderer} onChange={setRenderer} options={RENDERER_OPTIONS} />
        <span className="text-[11px] text-base-content/50">
          Click a group to see its row and files, and a file to see its functions.
        </span>
        {toolbarEnd && <span className="ml-auto">{toolbarEnd}</span>}
      </Toolbar>
      <ScrollTreeLayout
        key={view.level}
        prefix={PREFIX}
        items={items}
        resolve={resolveItem}
        beforeJump={openTreeItem}
        watchKey={`${renderer}:${view.groupId ?? ""}`}
        heading={heading}
      >
        {view.level === "summary" && (
          <GroupRows
            rows={[allRow]}
            renderer={renderer}
            handlers={handlers}
            anchor={(row) => anchorId(PREFIX, row.group.id)}
          />
        )}
        {view.level === "groups" && (
          <GroupRows
            rows={rows}
            renderer={renderer}
            handlers={handlers}
            anchor={(row) => anchorId(PREFIX, row.group.id)}
          />
        )}
        {view.level === "functions" && view.groupId !== null && (
          <FunctionsOfGroup
            group={findGroup(groups, view.groupId)}
            row={rows.find((row) => row.group.id === view.groupId)}
            files={filesInGroup(files, view.groupId)}
            renderer={renderer}
          />
        )}
      </ScrollTreeLayout>
    </div>
  );
}

function FunctionsOfGroup({
  group,
  row,
  files,
  renderer,
}: {
  group: GroupSummary;
  row: GroupRow | undefined;
  files: GroupedFile[];
  renderer: Renderer;
}) {
  return (
    <div>
      <div
        id={anchorId(PREFIX, group.id)}
        className="flex flex-wrap items-center gap-x-4 gap-y-1 border-b border-base-300 px-4 py-2 text-[11px]"
      >
        <span className="text-xs font-semibold">{group.label}</span>
        {row && <StatsLine stats={row.stats} />}
      </div>
      <div id={anchorId(PREFIX, "functions")}>
        <FunctionsView files={files} renderer={renderer} />
      </div>
    </div>
  );
}
