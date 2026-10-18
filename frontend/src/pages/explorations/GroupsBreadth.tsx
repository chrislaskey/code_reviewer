import { useState } from "react";
import { DocumentTextIcon, PlusIcon, XMarkIcon } from "@heroicons/react/16/solid";
import type { GroupSummary } from "../../api/types";
import { SegmentedControl, toggled } from "../../explorations/controls";
import {
  EXAMPLE_GROUPS,
  EXAMPLE_GROUPS_TOML,
  EXAMPLE_PATTERNS,
  GROUPED_FILES,
  UNGROUPED_ID,
  childGroups,
  filesInGroup,
  findGroup,
  findGroupLinks,
  summarizeFiles,
  type GroupedFile,
  type Stats,
} from "../../explorations/groups_data";
import {
  ChangeBar,
  FileSummaryLine,
  GroupPills,
  groupIcon,
  LinesLabel,
  NoGroupsNote,
  StaleIcon,
  StatsLine,
} from "../../explorations/groups_parts";
import { plural } from "../../explorations/nodes";
import { ExplorationPage, Option, Section } from "../../explorations/scaffold";
import { ScrollTreeLayout, anchorId, type TreeItem } from "../../explorations/scroll_tree";

const FILES = GROUPED_FILES;
const GROUPS = EXAMPLE_GROUPS;
const TOP_LEVEL = childGroups(GROUPS, null);
const LEAF_GROUPS = GROUPS.filter((group) => !group.is_parent);

function statsOf(groupId: string): Stats {
  return summarizeFiles(filesInGroup(FILES, groupId));
}

const MOST_LINES = Math.max(
  ...GROUPS.map((group) => {
    const stats = statsOf(group.id);
    return stats.added_lines + stats.removed_lines;
  }),
);

function groupTitle(group: GroupSummary) {
  return (
    <span className="flex items-baseline gap-2">
      <span className={`text-sm font-semibold ${group.id === UNGROUPED_ID ? "text-base-content/60" : ""}`}>
        {group.label}
      </span>
      <span className="font-mono text-[10px] text-base-content/40">{group.id}</span>
    </span>
  );
}

function FileLines({ files }: { files: GroupedFile[] }) {
  return (
    <ul className="divide-y divide-base-300">
      {files.map((file) => (
        <li key={file.id}>
          <FileSummaryLine file={file} />
        </li>
      ))}
    </ul>
  );
}

/* -- H1: group cards -------------------------------------------------------------- */

function GroupCard({ group }: { group: GroupSummary }) {
  const stats = statsOf(group.id);
  const subgroups = childGroups(GROUPS, group.id);
  const dashed = group.id === UNGROUPED_ID;
  return (
    <article
      className={[
        "flex flex-col rounded-xl border bg-base-100",
        dashed ? "border-dashed border-base-content/30" : "border-base-300",
      ].join(" ")}
    >
      <header className="space-y-2 border-b border-base-300 px-4 py-3">
        {groupTitle(group)}
        <ChangeBar stats={stats} max={MOST_LINES} />
        <StatsLine stats={stats} />
      </header>
      <div className="flex-1 divide-y divide-base-300">
        {subgroups.length > 0
          ? subgroups.map((subgroup) => (
              <div key={subgroup.id} className="space-y-1 px-4 py-2">
                <p className="text-xs font-medium">{subgroup.label}</p>
                <StatsLine stats={statsOf(subgroup.id)} />
              </div>
            ))
          : filesInGroup(FILES, group.id).map((file) => (
              <div key={file.id} className="px-4 py-2">
                <p className="truncate font-mono text-[11px]">{file.path}</p>
                <span className="font-mono text-[10px] text-base-content/50">
                  {plural(summarizeFiles([file]).function_count, "function")}
                </span>
              </div>
            ))}
      </div>
      <footer className="border-t border-base-300 px-4 py-2 text-right text-[11px] text-primary">
        {subgroups.length > 0 ? `Open ${subgroups.length} subgroups →` : "Open files →"}
      </footer>
    </article>
  );
}

function GroupCards() {
  return (
    <div className="grid gap-4 p-4 md:grid-cols-2 xl:grid-cols-4">
      {TOP_LEVEL.map((group) => (
        <GroupCard key={group.id} group={group} />
      ))}
    </div>
  );
}

/* -- H2: group table -------------------------------------------------------------- */

function depthOf(group: GroupSummary): number {
  return group.parent_id === null ? 0 : 1 + depthOf(findGroup(GROUPS, group.parent_id));
}

function GroupTable() {
  const total = summarizeFiles(FILES);
  return (
    <table className="w-full border-collapse bg-base-100 text-left">
      <thead>
        <tr className="border-b border-base-300 text-[10px] uppercase tracking-wide text-base-content/50">
          <th className="px-4 py-2 font-medium">Group</th>
          <th className="px-2 py-2 text-right font-medium">Files</th>
          <th className="w-48 px-2 py-2 font-medium">Lines</th>
          <th className="w-8 px-2 py-2" />
        </tr>
      </thead>
      <tbody>
        {GROUPS.map((group) => {
          const stats = statsOf(group.id);
          return (
            <tr
              key={group.id}
              className={[
                "border-b border-base-300 hover:bg-base-200/60",
                group.id === UNGROUPED_ID ? "text-base-content/60" : "",
              ].join(" ")}
            >
              <td className="py-2 pr-2 text-xs" style={{ paddingLeft: 16 + depthOf(group) * 20 }}>
                <span className={group.is_parent ? "font-semibold" : ""}>{group.label}</span>
              </td>
              <td className="px-2 py-2 text-right font-mono text-[11px]">{stats.file_count}</td>
              <td className="px-2 py-2">
                <div className="flex items-center gap-2">
                  <div className="w-24">
                    <ChangeBar stats={stats} max={MOST_LINES} />
                  </div>
                  <LinesLabel added={stats.added_lines} removed={stats.removed_lines} />
                </div>
              </td>
              <td className="px-2 py-2">
                <StaleIcon count={stats.stale_caller_count} />
              </td>
            </tr>
          );
        })}
      </tbody>
      <tfoot>
        <tr className="text-xs font-semibold">
          <td className="px-4 py-2">All changes</td>
          <td className="px-2 py-2 text-right font-mono text-[11px]">{total.file_count}</td>
          <td className="px-2 py-2">
            <LinesLabel added={total.added_lines} removed={total.removed_lines} />
          </td>
          <td className="px-2 py-2">
            <StaleIcon count={total.stale_caller_count} />
          </td>
        </tr>
      </tfoot>
    </table>
  );
}

/* -- H3: sections with file lines ------------------------------------------------------ */

function GroupSections() {
  return (
    <div className="space-y-6 p-4">
      {TOP_LEVEL.map((group) => {
        const subgroups = childGroups(GROUPS, group.id);
        const sections = subgroups.length > 0 ? subgroups : [group];
        return (
          <section key={group.id} className="overflow-hidden rounded-xl border border-base-300 bg-base-100">
            <header className="flex flex-wrap items-center justify-between gap-2 border-b border-base-300 bg-base-200/60 px-4 py-2">
              {groupTitle(group)}
              <StatsLine stats={statsOf(group.id)} />
            </header>
            {sections.map((section) => (
              <div key={section.id}>
                {subgroups.length > 0 && (
                  <p className="border-b border-base-300 bg-base-100 px-4 py-1.5 text-[11px] font-medium text-base-content/60">
                    {section.label}
                  </p>
                )}
                <FileLines files={filesInGroup(FILES, section.id)} />
              </div>
            ))}
          </section>
        );
      })}
    </div>
  );
}

/* -- H4: tree and summary ----------------------------------------------------------------- */

const H4_PREFIX = "h4-";

function treeItems(): TreeItem[] {
  return GROUPS.flatMap((group) => {
    const Icon = groupIcon(group);
    const own: TreeItem = {
      id: group.id,
      label: group.label,
      depth: depthOf(group),
      icon: <Icon className="size-3.5" />,
      detail: String(statsOf(group.id).file_count),
      dim: group.id === UNGROUPED_ID,
    };
    const files: TreeItem[] = group.is_parent
      ? []
      : filesInGroup(FILES, group.id).map((file) => ({
          id: `file:${group.id}:${file.id}`,
          label: file.path.split("/").pop() ?? file.path,
          depth: depthOf(group) + 1,
          icon: <DocumentTextIcon className="size-3.5" />,
          mono: true,
        }));
    return [own, ...files];
  });
}

const H4_ITEMS = treeItems();

function TreeAndSummary() {
  return (
    <ScrollTreeLayout prefix={H4_PREFIX} items={H4_ITEMS}>
      <div className="space-y-4 p-4">
        {GROUPS.map((group) => (
          <section
            key={group.id}
            id={anchorId(H4_PREFIX, group.id)}
            style={{ marginLeft: depthOf(group) * 24 }}
            className="overflow-hidden rounded-xl border border-base-300 bg-base-100"
          >
            <header className="flex flex-wrap items-center justify-between gap-2 border-b border-base-300 bg-base-200/60 px-4 py-2">
              {groupTitle(group)}
              <StatsLine stats={statsOf(group.id)} />
            </header>
            {group.is_parent ? (
              <p className="px-4 py-2 text-[11px] text-base-content/50">
                Holds the files of its subgroups.
              </p>
            ) : (
              <ul className="divide-y divide-base-300">
                {filesInGroup(FILES, group.id).map((file) => (
                  <li key={file.id} id={anchorId(H4_PREFIX, `file:${group.id}:${file.id}`)}>
                    <FileSummaryLine file={file} />
                  </li>
                ))}
              </ul>
            )}
          </section>
        ))}
      </div>
    </ScrollTreeLayout>
  );
}

/* -- H5: dependency matrix ------------------------------------------------------------------ */

function DependencyMatrix() {
  const links = findGroupLinks(
    FILES,
    LEAF_GROUPS.map((group) => group.id),
  );
  const countFor = (fromId: string, toId: string) =>
    links.find((link) => link.from_id === fromId && link.to_id === toId)?.call_count ?? 0;
  const highest = Math.max(...links.map((link) => link.call_count));
  return (
    <div className="overflow-x-auto bg-base-100 p-4">
      <table className="border-collapse text-xs">
        <thead>
          <tr>
            <th className="p-2 text-left text-[10px] font-medium uppercase tracking-wide text-base-content/50">
              calls from ↓ into →
            </th>
            {LEAF_GROUPS.map((group) => (
              <th key={group.id} className="w-24 p-2 text-center font-medium">
                {group.label}
              </th>
            ))}
          </tr>
        </thead>
        <tbody>
          {LEAF_GROUPS.map((from) => (
            <tr key={from.id}>
              <th className="p-2 text-left font-medium">{from.label}</th>
              {LEAF_GROUPS.map((to) => {
                const count = countFor(from.id, to.id);
                return (
                  <td
                    key={to.id}
                    className="border border-base-300 p-2 text-center font-mono"
                    style={{
                      backgroundColor:
                        count > 0
                          ? `color-mix(in oklab, var(--color-primary) ${20 + (count / highest) * 50}%, transparent)`
                          : undefined,
                    }}
                  >
                    {from.id === to.id ? (
                      <span className="text-base-content/20">·</span>
                    ) : count > 0 ? (
                      count
                    ) : (
                      ""
                    )}
                  </td>
                );
              })}
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}

/* -- H6: groups as filters ------------------------------------------------------------------ */

function GroupFilters() {
  const [active, setActive] = useState<Set<string>>(new Set());
  const shown = active.size === 0 ? FILES : FILES.filter((file) => file.group_ids.some((id) => active.has(id)));
  return (
    <div className="bg-base-100">
      <div className="flex flex-wrap items-center gap-2 border-b border-base-300 px-4 py-2.5">
        {GROUPS.map((group) => {
          const on = active.has(group.id);
          return (
            <button
              key={group.id}
              type="button"
              onClick={() => setActive(toggled(active, group.id))}
              className={[
                "rounded-full border px-2.5 py-0.5 text-[11px]",
                on
                  ? "border-primary bg-primary/15 font-semibold"
                  : "border-base-300 text-base-content/70 hover:border-base-content/40",
                group.id === UNGROUPED_ID ? "border-dashed" : "",
              ].join(" ")}
            >
              {group.parent_id ? `${findGroup(GROUPS, group.parent_id).label} / ` : ""}
              {group.label}{" "}
              <span className="font-mono text-[10px] text-base-content/50">{statsOf(group.id).file_count}</span>
            </button>
          );
        })}
        {active.size > 0 && (
          <button
            type="button"
            onClick={() => setActive(new Set())}
            className="text-[11px] text-primary hover:underline"
          >
            Clear
          </button>
        )}
      </div>
      <ul className="divide-y divide-base-300">
        {shown.map((file) => (
          <li key={file.id}>
            <FileSummaryLine file={file} trailing={<GroupPills file={file} groups={GROUPS} />} />
          </li>
        ))}
      </ul>
      <NoGroupsNote>
        Showing {plural(shown.length, "file")}. A file shows every group it is in, and selecting two groups shows files in either.
      </NoGroupsNote>
    </div>
  );
}

/* -- Opt in -------------------------------------------------------------------------------- */

function OptInToggle() {
  const [mode, setMode] = useState<"none" | "defined">("none");
  return (
    <div>
      <div className="flex items-center gap-3 border-b border-base-300 bg-base-100 px-4 py-2">
        <SegmentedControl
          label="Groups"
          value={mode}
          onChange={setMode}
          options={[
            { value: "none", label: "None defined" },
            { value: "defined", label: "Defined" },
          ]}
        />
      </div>
      {mode === "none" ? (
        <div className="bg-base-100">
          <div className="flex items-center justify-between gap-4 border-b border-dashed border-base-content/30 px-4 py-3">
            <p className="text-xs text-base-content/70">
              No groups yet. Files are listed as they are today. <span className="text-base-content/50">No “Ungrouped” bucket is shown.</span>
            </p>
            <button type="button" className="rounded-lg border border-base-300 px-3 py-1 text-xs hover:bg-base-200">
              Set up groups…
            </button>
          </div>
          <FileLines files={FILES} />
        </div>
      ) : (
        <GroupCards />
      )}
    </div>
  );
}

/* -- Defining groups ------------------------------------------------------------------------- */

function MatchedFiles({ group }: { group: GroupSummary }) {
  const stats = statsOf(group.id);
  return (
    <span className="font-mono text-[10px] text-base-content/50">{plural(stats.file_count, "file")}</span>
  );
}

function ConfigWithPreview() {
  const ungrouped = filesInGroup(FILES, UNGROUPED_ID);
  return (
    <div className="grid gap-px bg-base-300 lg:grid-cols-2">
      <div className="space-y-3 bg-base-100 p-4">
        <p className="text-[11px] font-medium uppercase tracking-wide text-base-content/50">
          .code_reviewer.toml
        </p>
        <pre className="max-h-80 overflow-auto rounded-lg border border-base-300 bg-base-200/60 p-3 font-mono text-[11px] leading-relaxed">
          {EXAMPLE_GROUPS_TOML}
        </pre>
        <p className="text-[11px] font-medium uppercase tracking-wide text-base-content/50">Flags</p>
        <pre className="overflow-auto rounded-lg border border-base-300 bg-base-200/60 p-3 font-mono text-[11px]">
          {`--group-include tests=spec/**\n--group-exclude core/business=lib/shop/legacy.ex`}
        </pre>
      </div>
      <div className="space-y-3 bg-base-100 p-4">
        <p className="text-[11px] font-medium uppercase tracking-wide text-base-content/50">
          What it matches
        </p>
        <ul className="space-y-1">
          {GROUPS.map((group) => (
            <li
              key={group.id}
              style={{ paddingLeft: depthOf(group) * 20 }}
              className="flex items-center justify-between gap-3 text-xs"
            >
              <span className={group.is_parent ? "font-semibold" : ""}>{group.label}</span>
              <MatchedFiles group={group} />
            </li>
          ))}
        </ul>
        <div className="rounded-lg border border-dashed border-base-content/30 p-3">
          <p className="mb-1 text-[11px] font-medium text-base-content/60">
            {plural(ungrouped.length, "file")} match no group
          </p>
          <ul className="font-mono text-[11px] text-base-content/70">
            {ungrouped.map((file) => (
              <li key={file.id}>{file.path}</li>
            ))}
          </ul>
        </div>
        <p className="rounded-lg border border-rose-500/40 bg-rose-500/10 px-3 py-2 text-[11px] text-rose-700 dark:text-rose-300">
          Error example: group “core” has subgroups, so it cannot have include or exclude patterns of its own.
        </p>
      </div>
    </div>
  );
}

function PatternChips({
  patterns,
  tone,
  onRemove,
}: {
  patterns: string[];
  tone: "include" | "exclude";
  onRemove: (pattern: string) => void;
}) {
  return (
    <span className="inline-flex flex-wrap items-center gap-1.5">
      {patterns.map((pattern) => (
        <span
          key={pattern}
          className={[
            "inline-flex items-center gap-1 rounded border px-1.5 py-0.5 font-mono text-[11px]",
            tone === "include"
              ? "border-emerald-500/40 bg-emerald-500/10"
              : "border-rose-500/40 bg-rose-500/10 line-through decoration-rose-500/60",
          ].join(" ")}
        >
          {pattern}
          <button type="button" onClick={() => onRemove(pattern)} title="Remove pattern">
            <XMarkIcon className="size-3 text-base-content/50 hover:text-base-content" />
          </button>
        </span>
      ))}
    </span>
  );
}

function GroupBuilder() {
  const [patterns, setPatterns] = useState(EXAMPLE_PATTERNS);
  const remove = (id: string, kind: "include" | "exclude", pattern: string) =>
    setPatterns((current) => ({
      ...current,
      [id]: { ...current[id], [kind]: current[id][kind].filter((item) => item !== pattern) },
    }));

  return (
    <div className="space-y-2 bg-base-100 p-4">
      {GROUPS.filter((group) => group.id !== UNGROUPED_ID).map((group) => {
        const own = patterns[group.id];
        return (
          <div
            key={group.id}
            style={{ marginLeft: depthOf(group) * 28 }}
            className="space-y-2 rounded-lg border border-base-300 px-3 py-2.5"
          >
            <div className="flex items-center justify-between gap-3">
              <span className="text-sm font-semibold">{group.label}</span>
              <span className="flex items-center gap-3">
                <MatchedFiles group={group} />
                <button type="button" className="text-[11px] text-primary hover:underline">
                  + subgroup
                </button>
              </span>
            </div>
            {group.is_parent ? (
              <p className="text-[11px] text-base-content/50">
                Holds every file of its subgroups. A group with subgroups has no patterns of its own.
              </p>
            ) : (
              <div className="space-y-1.5">
                <div className="flex flex-wrap items-center gap-2">
                  <span className="w-14 text-[10px] uppercase tracking-wide text-base-content/50">Include</span>
                  <PatternChips patterns={own.include} tone="include" onRemove={(p) => remove(group.id, "include", p)} />
                  <button type="button" className="inline-flex items-center text-[11px] text-primary">
                    <PlusIcon className="size-3" /> add
                  </button>
                </div>
                <div className="flex flex-wrap items-center gap-2">
                  <span className="w-14 text-[10px] uppercase tracking-wide text-base-content/50">Exclude</span>
                  <PatternChips patterns={own.exclude} tone="exclude" onRemove={(p) => remove(group.id, "exclude", p)} />
                  <button type="button" className="inline-flex items-center text-[11px] text-primary">
                    <PlusIcon className="size-3" /> add
                  </button>
                </div>
              </div>
            )}
          </div>
        );
      })}
      <button type="button" className="rounded-lg border border-dashed border-base-content/30 px-3 py-1.5 text-xs text-base-content/70 hover:bg-base-200">
        + New group
      </button>
    </div>
  );
}

/* -- Page --------------------------------------------------------------------------------------- */

export default function GroupsBreadth() {
  return (
    <ExplorationPage
      title="Groups, breadth"
      intro={
        <>
          <p>
            With groups defined, the top of the review is groups and their files instead of
            functions. These are different ways to draw that top level, using one hardcoded
            example: seven groups over twelve files, including the implicit Ungrouped one.
          </p>
          <p>
            A file can be in many groups, so a group's numbers are not exclusive. A parent group
            is only a roll-up of its subgroups. Groups are opt in: with none defined nothing is
            grouped. Nothing here is wired to the backend.
          </p>
        </>
      }
    >
      <Section
        id="top-level"
        title="Top level: groups and files"
        intro="Which group needs attention, and how big is each? No function-level detail yet."
      >
        <Option
          code="H1"
          title="Group cards"
          note="One card per top-level group: a size bar, the numbers, and a peek at its subgroups or files. Friendly, shows the shape of the change at a glance. Cards take space, so ten groups need scrolling."
        >
          <GroupCards />
        </Option>
        <Option
          code="H2"
          title="Group table"
          note="Every group on one line, subgroups indented, with an all-changes total at the bottom that counts each file once. The densest view and easiest to compare, but the least inviting to click into."
        >
          <GroupTable />
        </Option>
        <Option
          code="H3"
          title="Sections with file lines"
          note="Every group is a section and every file is a summary line under it. All files are visible without a click. Long for big changes, and a file in two groups appears twice."
        >
          <GroupSections />
        </Option>
        <Option
          code="H4"
          title="Tree and summary"
          clip={false}
          note="The groups and their files as a tree on the left, and every group on one long page on the right. Clicking an item scrolls the page to it, like a GitHub pull request, and the tree highlights the section you are in. Scroll the page to see it follow."
        >
          <TreeAndSummary />
        </Option>
        <Option
          code="H5"
          title="Dependency matrix"
          note="Who calls whom between groups, as call counts. Answers 'who depends on core/data?', which no other idea does. Says nothing about size or status, so it is a companion, not a front page."
        >
          <DependencyMatrix />
        </Option>
        <Option
          code="H6"
          title="Groups as filters"
          note="No levels: one flat file list, with groups as toggle pills above it and group tags on each file. Handles many-to-many naturally and is the smallest change from today. Loses the summary-per-group view."
        >
          <GroupFilters />
        </Option>
      </Section>

      <Section
        id="opt-in"
        title="Opt in"
        intro="Groups are off until someone defines one, and Ungrouped never shows up alone."
      >
        <Option
          code="O1"
          title="None defined, then defined"
          note="With no groups the page is the flat file view of today, with a quiet prompt to set groups up. Once any group exists, everything not matched appears in a dashed Ungrouped card."
        >
          <OptInToggle />
        </Option>
      </Section>

      <Section
        id="defining"
        title="Defining groups"
        intro="The same definitions as the config file and flags, entered or checked in the UI."
      >
        <Option
          code="S1"
          title="Config text with a live preview"
          note="The TOML file and the flags as text, next to what they match: file counts per group, the files in no group, and the error text a bad config produces. Closest to the real file, so it teaches the format."
        >
          <ConfigWithPreview />
        </Option>
        <Option
          code="S2"
          title="Group builder"
          note="A form: a card per group with include and exclude patterns as removable chips. A parent group shows that it has no patterns of its own. Easier for newcomers, and it would write the same TOML. Counts are hardcoded here."
        >
          <GroupBuilder />
        </Option>
      </Section>
    </ExplorationPage>
  );
}
