import { useState, type ReactNode } from "react";
import { ChevronDownIcon, ChevronRightIcon, DocumentTextIcon } from "@heroicons/react/16/solid";
import { Link } from "react-router-dom";
import type { GroupSummary } from "../../api/types";
import { SegmentedControl, Toolbar, toggled } from "../../explorations/controls";
import {
  EXAMPLE_GROUPS,
  GROUPED_FILES,
  UNGROUPED_ID,
  childGroups,
  filesInGroup,
  findGroup,
  findOutsideLinks,
  groupPath,
  summarizeFiles,
  type GroupedFile,
} from "../../explorations/groups_data";
import {
  FileSummaryLine,
  GroupPills,
  StatsLine,
  groupIcon,
  ZoomedReview,
} from "../../explorations/groups_parts";
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

/* -- Levels --------------------------------------------------------------------- */

/**
 * Where the reader is. No group is the top level (groups and their files), a parent
 * group lists its subgroups, a leaf group shows the file and function view. A file
 * narrows the leaf view to that file.
 */
interface Selection {
  groupId: string | null;
  fileId: string | null;
}

const TOP: Selection = { groupId: null, fileId: null };

type Open = (groupId: string | null, fileId?: string | null) => void;

function statsOf(groupId: string) {
  return summarizeFiles(filesInGroup(FILES, groupId));
}

/** The leaf group of a file inside a section, so a click knows where to zoom. */
function leafWithin(file: GroupedFile, sectionId: string): string {
  const leaf = file.group_ids.find((id) => {
    const group = findGroup(GROUPS, id);
    return !group.is_parent && (id === sectionId || id.startsWith(`${sectionId}/`));
  });
  return leaf ?? sectionId;
}

function SectionHeader({
  group,
  onOpen,
  lead,
  hideAction = false,
}: {
  group: GroupSummary;
  onOpen?: () => void;
  lead?: ReactNode;
  hideAction?: boolean;
}) {
  const stats = statsOf(group.id);
  const body = (
    <>
      {lead}
      <span className="flex min-w-0 flex-1 flex-wrap items-baseline gap-x-3 gap-y-1">
        <span className={`text-sm font-semibold ${group.id === UNGROUPED_ID ? "text-base-content/60" : ""}`}>
          {group.label}
        </span>
        <StatsLine stats={stats} />
      </span>
      {onOpen && !hideAction && (
        <span className="shrink-0 text-[11px] text-primary">
          {group.is_parent ? "Open subgroups →" : "Open functions →"}
        </span>
      )}
    </>
  );
  const className =
    "flex w-full items-center gap-3 border-b border-base-300 bg-base-200/60 px-4 py-2 text-left";
  return onOpen ? (
    <button type="button" onClick={onOpen} className={`${className} hover:bg-base-200`}>
      {body}
    </button>
  ) : (
    <div className={className}>{body}</div>
  );
}

/** One group and the summary line of every file in it. */
function GroupSection({ group, onOpen }: { group: GroupSummary; onOpen: Open }) {
  const files = filesInGroup(FILES, group.id);
  return (
    <section
      className={[
        "overflow-hidden rounded-xl border bg-base-100",
        group.id === UNGROUPED_ID ? "border-dashed border-base-content/30" : "border-base-300",
      ].join(" ")}
    >
      <SectionHeader group={group} onOpen={() => onOpen(group.id)} />
      <ul className="divide-y divide-base-300">
        {files.map((file) => (
          <li key={file.id}>
            <FileSummaryLine
              file={file}
              trailing={group.is_parent ? <GroupPills file={file} groups={GROUPS} /> : undefined}
              onClick={() => onOpen(leafWithin(file, group.id), file.id)}
            />
          </li>
        ))}
      </ul>
    </section>
  );
}

function GroupSections({ groups, onOpen }: { groups: GroupSummary[]; onOpen: Open }) {
  return (
    <div className="space-y-4 p-4">
      {groups.map((group) => (
        <GroupSection key={group.id} group={group} onOpen={onOpen} />
      ))}
    </div>
  );
}

/** Which other groups call into this one, and which it calls. Each one is a way to jump. */
function OutsideLinks({ groupId, onOpen }: { groupId: string; onOpen: Open }) {
  const { incoming, outgoing } = findOutsideLinks(FILES, GROUPS, groupId);
  if (incoming.length === 0 && outgoing.length === 0) return null;
  const list = (items: { group_id: string; call_count: number }[], label: string) =>
    items.length > 0 && (
      <span className="flex flex-wrap items-center gap-1.5">
        <span className="text-base-content/50">{label}</span>
        {items.map((item) => (
          <button
            key={item.group_id}
            type="button"
            onClick={() => onOpen(item.group_id)}
            className="rounded-full border border-base-300 bg-base-100 px-2 py-px hover:border-primary"
          >
            {findGroup(GROUPS, item.group_id).label}{" "}
            <span className="font-mono text-[10px] text-base-content/50">×{item.call_count}</span>
          </button>
        ))}
      </span>
    );
  return (
    <div className="flex flex-wrap items-center gap-x-6 gap-y-1 border-b border-base-300 bg-base-100 px-4 py-2 text-[11px]">
      {list(incoming, "Called from")}
      {list(outgoing, "Calls into")}
    </div>
  );
}

/** The file and function view: the review as it is today, for the files of one group. */
function LeafView({ group, fileId, onOpen }: { group: GroupSummary; fileId: string | null; onOpen: Open }) {
  const files = filesInGroup(FILES, group.id);
  const shown = fileId ? files.filter((file) => file.id === fileId) : files;
  return (
    <div>
      {fileId && (
        <p className="flex items-center gap-3 border-b border-base-300 bg-base-100 px-4 py-2 text-[11px] text-base-content/70">
          Showing {shown[0]?.path} only
          <button type="button" onClick={() => onOpen(group.id, null)} className="text-primary hover:underline">
            Show all {plural(files.length, "file")} in {group.label}
          </button>
        </p>
      )}
      <ZoomedReview files={shown} />
    </div>
  );
}

/** Whatever the selection points at, drawn at its own level of detail. */
function Level({ selection, onOpen }: { selection: Selection; onOpen: Open }) {
  if (selection.groupId === null) return <GroupSections groups={TOP_LEVEL} onOpen={onOpen} />;
  const group = findGroup(GROUPS, selection.groupId);
  return (
    <div>
      <SectionHeader group={group} />
      <OutsideLinks groupId={group.id} onOpen={onOpen} />
      {group.is_parent ? (
        <GroupSections groups={childGroups(GROUPS, group.id)} onOpen={onOpen} />
      ) : (
        <LeafView group={group} fileId={selection.fileId} onOpen={onOpen} />
      )}
    </div>
  );
}

function useSelection(): [Selection, Open] {
  const [selection, setSelection] = useState<Selection>(TOP);
  const open: Open = (groupId, fileId = null) => setSelection({ groupId, fileId });
  return [selection, open];
}

/* -- D1: drill down with a breadcrumb ----------------------------------------------- */

function Breadcrumb({ selection, onOpen }: { selection: Selection; onOpen: Open }) {
  const trail = selection.groupId ? groupPath(GROUPS, selection.groupId) : [];
  const file = FILES.find((candidate) => candidate.id === selection.fileId);
  const siblings = selection.groupId
    ? childGroups(GROUPS, findGroup(GROUPS, selection.groupId).parent_id)
    : [];
  const crumb = "text-primary hover:underline";
  return (
    <div className="space-y-2 border-b border-base-300 bg-base-100 px-4 py-2.5">
      <nav className="flex flex-wrap items-center gap-1.5 text-xs">
        <button type="button" onClick={() => onOpen(null)} className={trail.length > 0 ? crumb : "font-semibold"}>
          All changes
        </button>
        {trail.map((group, index) => {
          const last = index === trail.length - 1 && !file;
          return (
            <span key={group.id} className="flex items-center gap-1.5">
              <ChevronRightIcon className="size-3 text-base-content/40" />
              <button type="button" onClick={() => onOpen(group.id)} className={last ? "font-semibold" : crumb}>
                {group.label}
              </button>
            </span>
          );
        })}
        {file && (
          <span className="flex items-center gap-1.5">
            <ChevronRightIcon className="size-3 text-base-content/40" />
            <span className="font-mono font-semibold">{file.path}</span>
          </span>
        )}
      </nav>
      {siblings.length > 1 && (
        <div className="flex flex-wrap gap-1.5">
          {siblings.map((sibling) => (
            <button
              key={sibling.id}
              type="button"
              onClick={() => onOpen(sibling.id)}
              className={[
                "rounded-full border px-2.5 py-0.5 text-[11px]",
                sibling.id === selection.groupId
                  ? "border-primary bg-primary/15 font-semibold"
                  : "border-base-300 text-base-content/70 hover:border-base-content/40",
              ].join(" ")}
            >
              {sibling.label}{" "}
              <span className="font-mono text-[10px] text-base-content/50">{statsOf(sibling.id).file_count}</span>
            </button>
          ))}
        </div>
      )}
    </div>
  );
}

function DrillDown() {
  const [selection, open] = useSelection();
  const [mode, setMode] = useState<"defined" | "none">("defined");
  return (
    <div>
      <Toolbar>
        <SegmentedControl
          label="Groups"
          value={mode}
          onChange={setMode}
          options={[
            { value: "defined", label: "Defined" },
            { value: "none", label: "None defined" },
          ]}
        />
        <span className="text-[11px] text-base-content/50">
          With none, the first screen is the file and function view with no group chrome.
        </span>
      </Toolbar>
      {mode === "none" ? (
        <ZoomedReview files={FILES} />
      ) : (
        <>
          <Breadcrumb selection={selection} onOpen={open} />
          <Level selection={selection} onOpen={open} />
        </>
      )}
    </div>
  );
}

/* -- D2: tree and levels --------------------------------------------------------------- */

function depthOf(group: GroupSummary): number {
  return group.parent_id === null ? 0 : 1 + depthOf(findGroup(GROUPS, group.parent_id));
}

const D2_PREFIX = "d2-";
const FILE_ITEM_PREFIX = "file:";

function d2FileItemId(groupId: string, fileId: string): string {
  return `${FILE_ITEM_PREFIX}${groupId}:${fileId}`;
}

const D2_ITEMS: TreeItem[] = GROUPS.flatMap((group) => {
  const Icon = groupIcon(group);
  const groupItem: TreeItem = {
    id: group.id,
    label: group.label,
    depth: depthOf(group),
    icon: <Icon className="size-3.5" />,
    detail: String(statsOf(group.id).file_count),
    dim: group.id === UNGROUPED_ID,
  };
  const fileItems: TreeItem[] = group.is_parent
    ? []
    : filesInGroup(FILES, group.id).map((file) => ({
        id: d2FileItemId(group.id, file.id),
        label: file.path.split("/").pop() ?? file.path,
        depth: depthOf(group) + 1,
        icon: <DocumentTextIcon className="size-3.5" />,
        mono: true,
      }));
  return [groupItem, ...fileItems];
});

/** A file's functions sit under its group's file list. */
function resolveD2Item(item: TreeItem): Element | null {
  if (!item.id.startsWith(FILE_ITEM_PREFIX)) {
    return document.getElementById(anchorId(D2_PREFIX, item.id));
  }
  const [groupId, fileId] = item.id.slice(FILE_ITEM_PREFIX.length).split(":");
  return document
    .getElementById(anchorId(D2_PREFIX, `functions:${groupId}`))
    ?.querySelector(`#review-file-${fileId}`) ?? null;
}

/** All groups and files on one page, then the functions of each leaf group. The tree scrolls to any of them. */
function TreeAndLevels() {
  const jump: Open = (groupId, fileId = null) => {
    if (groupId === null) return;
    const group = findGroup(GROUPS, groupId);
    const target = fileId
      ? d2FileItemId(groupId, fileId)
      : group.is_parent
        ? (childGroups(GROUPS, groupId)[0]?.id ?? groupId)
        : `functions:${groupId}`;
    const element = target.startsWith("functions:")
      ? document.getElementById(anchorId(D2_PREFIX, target))
      : resolveD2Item({ id: target, label: "", depth: 0 });
    scrollToElement(element);
  };
  return (
    <ScrollTreeLayout prefix={D2_PREFIX} items={D2_ITEMS} resolve={resolveD2Item}>
      <div className="space-y-8 p-4">
        {GROUPS.map((group) => {
          const files = filesInGroup(FILES, group.id);
          return (
            <div key={group.id} style={{ marginLeft: depthOf(group) * 16 }} className="space-y-3">
              <div id={anchorId(D2_PREFIX, group.id)}>
                <GroupSection group={group} onOpen={jump} />
              </div>
              {!group.is_parent && files.length > 0 && (
                <div
                  id={anchorId(D2_PREFIX, `functions:${group.id}`)}
                  className="overflow-x-auto rounded-xl border border-base-300"
                >
                  <p className="border-b border-base-300 bg-base-100 px-4 py-1.5 text-[11px] font-medium text-base-content/60">
                    Functions of {group.label}
                  </p>
                  <ZoomedReview files={files} />
                </div>
              )}
            </div>
          );
        })}
      </div>
    </ScrollTreeLayout>
  );
}

/* -- D3: expand in place ------------------------------------------------------------------ */

type Zoom = "groups" | "files" | "functions";

const ALL_GROUP_KEYS = GROUPS.map((group) => `group:${group.id}`);

function fileKey(groupId: string, file: GroupedFile): string {
  return `file:${groupId}:${file.id}`;
}

const ALL_FILE_KEYS = GROUPS.filter((group) => !group.is_parent).flatMap((group) =>
  filesInGroup(FILES, group.id).map((file) => fileKey(group.id, file)),
);

function openFor(zoom: Zoom): Set<string> {
  if (zoom === "groups") return new Set();
  return new Set(zoom === "files" ? ALL_GROUP_KEYS : [...ALL_GROUP_KEYS, ...ALL_FILE_KEYS]);
}

function Chevron({ open }: { open: boolean }) {
  const Icon = open ? ChevronDownIcon : ChevronRightIcon;
  return <Icon className="size-4 shrink-0 text-base-content/50" />;
}

function OutlineGroup({
  group,
  depth,
  open,
  onToggle,
}: {
  group: GroupSummary;
  depth: number;
  open: Set<string>;
  onToggle: (key: string) => void;
}) {
  const key = `group:${group.id}`;
  const isOpen = open.has(key);
  return (
    <div className={depth === 0 ? "overflow-hidden rounded-xl border border-base-300 bg-base-100" : "border-t border-base-300"}>
      <SectionHeader
        group={group}
        onOpen={() => onToggle(key)}
        lead={<Chevron open={isOpen} />}
        hideAction
      />
      {isOpen &&
        (group.is_parent ? (
          <div className="pl-6">
            {childGroups(GROUPS, group.id).map((child) => (
              <OutlineGroup key={child.id} group={child} depth={depth + 1} open={open} onToggle={onToggle} />
            ))}
          </div>
        ) : (
          <ul className="divide-y divide-base-300">
            {filesInGroup(FILES, group.id).map((file) => {
              const fileOpen = open.has(fileKey(group.id, file));
              return (
                <li key={file.id}>
                  <FileSummaryLine
                    file={file}
                    onClick={() => onToggle(fileKey(group.id, file))}
                    leading={<Chevron open={fileOpen} />}
                  />
                  {fileOpen && (
                    <div className="overflow-x-auto border-t border-base-300 bg-base-200/40">
                      <ZoomedReview files={[file]} />
                    </div>
                  )}
                </li>
              );
            })}
          </ul>
        ))}
    </div>
  );
}

function ExpandInPlace() {
  const [zoom, setZoom] = useState<Zoom>("files");
  const [open, setOpen] = useState<Set<string>>(() => openFor("files"));
  const chooseZoom = (next: Zoom) => {
    setZoom(next);
    setOpen(openFor(next));
  };
  return (
    <div>
      <Toolbar>
        <SegmentedControl
          label="Zoom everything"
          value={zoom}
          onChange={chooseZoom}
          options={[
            { value: "groups", label: "Groups" },
            { value: "files", label: "Files" },
            { value: "functions", label: "Functions" },
          ]}
        />
        <span className="text-[11px] text-base-content/50">
          Or click any group or file to open just that one.
        </span>
      </Toolbar>
      <div className="space-y-4 p-4">
        {TOP_LEVEL.map((group) => (
          <OutlineGroup
            key={group.id}
            group={group}
            depth={0}
            open={open}
            onToggle={(key) => setOpen((current) => toggled(current, key))}
          />
        ))}
      </div>
    </div>
  );
}

/* -- Page ------------------------------------------------------------------------------------ */

export default function GroupsDepth() {
  return (
    <ExplorationPage
      title="Groups, depth"
      intro={
        <>
          <p>
            Three ways to move between the levels of a grouped review. The top level is groups
            and their files, with no function detail. A parent group opens into its subgroups,
            and a leaf group opens into the file and function view the app has today.
          </p>
          <p>
            All three are clickable and use the hardcoded example from the breadth page. Try
            Web › LiveViews, then use the Called from and Calls into pills to hop across groups.
          </p>
          <p>
            These use three tiers (groups, files, functions). The next round,{" "}
            <Link to="/explorations/groups-flow" className="text-primary hover:underline">
              Groups, flow
            </Link>
            , replaces the file tier with a Groups view built like the Functions view.
          </p>
        </>
      }
    >
      <Section
        id="navigation"
        title="Moving across levels"
        intro="The same three levels and the same content. What differs is how you move and what stays on screen."
      >
        <Option
          code="D1"
          title="Drill down with a breadcrumb"
          note="One level on screen at a time. Click a group header to go in, the breadcrumb to go back out, and sibling pills to move sideways. Clicking a file goes straight to the functions view narrowed to that file. Called from and Calls into pills hop between groups by dependency. Simplest and the calmest; you only see the neighbours you pass through."
        >
          <DrillDown />
        </Option>
        <Option
          code="D2"
          title="Tree that scrolls"
          clip={false}
          note="The group tree stays on the left while the page scrolls, like the file tree on a GitHub pull request. Every group and its files are on the page, with the functions of each leaf group below them. Clicking an item scrolls to it and the tree highlights where you are. Nothing is hidden."
        >
          <TreeAndLevels />
        </Option>
        <Option
          code="D3"
          title="Expand in place"
          note="One long page. Groups open into subgroups and files, and files open into their functions. A zoom control opens everything to one level at once, and clicks open single nodes, so you can see a whole area at file level and one file at function level. Never leaves the page, but long and easy to lose your place."
        >
          <ExpandInPlace />
        </Option>
      </Section>
    </ExplorationPage>
  );
}
