import { useEffect, useMemo, useRef, useState } from "react";
import { ChevronDownIcon, ChevronRightIcon } from "@heroicons/react/16/solid";
import { Card, CardHeader, StartEllipsis } from "../components/card";
import { ChipColumn, Connector, HtmlReview } from "../review/html/HtmlReview";
import { FlowActionsContext, type FlowActions } from "./actions";
import type { ExampleFile } from "./data";
import type { GroupLine, GroupLink, GroupRow } from "./group_rows";
import { ALL_CHANGES_ID, UNGROUPED_ID } from "./groups_data";
import { LinesLabel, StaleIcon, groupIcon } from "./groups_parts";
import { SIZES } from "./layout";
import { LedgerLine, plural, type ChipData, type Marker } from "./nodes";
import { CHANGE_STYLES } from "./style";

/*
 * The Groups view: the file view one level up. Header is a group, content is its
 * files, and Outputs and Inputs are the files outside the group.
 * Ledger and node renderers mirror the ones the Functions view already has.
 */

export type Renderer = "ledger" | "nodes";

export const RENDERER_OPTIONS: { value: Renderer; label: string }[] = [
  { value: "ledger", label: "Ledger" },
  { value: "nodes", label: "Nodes" },
];

export interface GroupRowHandlers {
  /** A group line (All changes row) scrolls to that group. A file line opens the functions of the group. */
  onOpenLine: (row: GroupRow, line: GroupLine) => void;
  /** A link to another changed file opens that file's group. */
  onOpenLink: (link: GroupLink) => void;
}

const LEDGER_WIDTH = 1040;
const COLUMN_WIDTHS = [400, 270, 270] as const;
const LINE_HEIGHT = 30;
const STACKED_LINE_HEIGHT = 40;

function shortName(label: string): string {
  return label.split("/").pop() ?? label;
}

function linkMarkers(link: GroupLink): Marker[] {
  const markers: Marker[] = [];
  if (link.is_stale) markers.push("stale");
  if (link.is_test) markers.push("test");
  if (link.file_id) markers.push("also_changed");
  return markers;
}

function linkSubtitle(link: GroupLink): string {
  const arrow = link.side === "output" ? `from ${shortName(link.line_label)}` : `→ ${shortName(link.line_label)}`;
  return `${link.group_label} · ${arrow}`;
}

function groupSubtitle(row: GroupRow): string {
  if (row.group.id === ALL_CHANGES_ID) return "every group";
  return row.group.id === UNGROUPED_ID ? "files in no group" : row.group.id;
}

/** The inside of a content line: a file or a group, with just its size and lines changed. */
function LineBody({ line }: { line: GroupLine }) {
  const { stats } = line;
  const size = line.kind === "group" ? plural(stats.file_count, "file") : plural(stats.function_count, "function");
  const name =
    line.kind === "group" ? (
      <span className="min-w-0 flex-1 truncate font-semibold">{line.label}</span>
    ) : (
      <span className="min-w-0 flex-1">
        <StartEllipsis>{line.label}</StartEllipsis>
      </span>
    );
  const status = line.status ? CHANGE_STYLES[line.status] : null;
  return (
    <>
      {status && (
        <span className={`w-3 shrink-0 text-center font-bold ${status.text}`}>{status.sign}</span>
      )}
      {name}
      <span className="shrink-0 text-[10px] text-base-content/50">{size}</span>
      <LinesLabel added={stats.added_lines} removed={stats.removed_lines} />
      <StaleIcon count={stats.stale_caller_count} />
      <ChevronRightIcon className="size-3 shrink-0 text-base-content/30" />
    </>
  );
}

function lineTitle(line: GroupLine): string {
  return line.kind === "group" ? `Open ${line.label}` : `Open the functions of ${line.label}`;
}

/* -- Ledger -------------------------------------------------------------------- */

function ColumnLabel({ children }: { children: string }) {
  return (
    <div className="flex h-5 items-center text-[10px] font-semibold uppercase tracking-wide text-base-content/40">
      {children}
    </div>
  );
}

function EmptyLine({ children }: { children: string }) {
  return (
    <div
      style={{ height: LINE_HEIGHT }}
      className="flex items-center px-2 font-mono text-[10px] text-base-content/40"
    >
      {children}
    </div>
  );
}

/** Hover a line to highlight its outputs and inputs and fade the rest. Same as the file ledger. */
export function GroupLedgerRow({ row, handlers }: { row: GroupRow; handlers: GroupRowHandlers }) {
  const [hovered, setHovered] = useState<string | null>(null);
  const [collapsed, setCollapsed] = useState(false);
  const status = CHANGE_STYLES[row.status];
  const Chevron = collapsed ? ChevronRightIcon : ChevronDownIcon;
  const stateOf = (key: string) => ({
    faded: hovered !== null && hovered !== key,
    highlighted: hovered === key,
  });

  const renderLink = (link: GroupLink) => (
    <LedgerLine
      key={link.id}
      label={link.path}
      detail={link.call_count > 1 ? `×${link.call_count}` : undefined}
      markers={linkMarkers(link)}
      tone={link.is_stale ? "stale" : "neutral"}
      dim={link.is_test}
      height={STACKED_LINE_HEIGHT}
      onHover={(on) => setHovered(on ? link.line_key : null)}
      onClick={link.file_id ? () => handlers.onOpenLink(link) : undefined}
      {...stateOf(link.line_key)}
    >
      <span className="min-w-0 flex-1 leading-4">
        <span className="block">
          <StartEllipsis>{link.path}</StartEllipsis>
        </span>
        <span className="block truncate text-[10px] text-base-content/50">{linkSubtitle(link)}</span>
      </span>
    </LedgerLine>
  );

  return (
    <div
      style={{ width: LEDGER_WIDTH }}
      className="overflow-hidden rounded-xl border border-base-300 bg-base-100 shadow-sm"
    >
      <header
        onClick={() => setCollapsed((current) => !current)}
        className="flex h-[52px] cursor-pointer items-center gap-3 px-4"
      >
        <Chevron className="size-4 text-base-content/50" />
        <span className={`size-2.5 rounded-full ${status.dot}`} />
        <h3 className="text-sm font-semibold">{row.group.label}</h3>
        <span className="font-mono text-[11px] text-base-content/50">{groupSubtitle(row)}</span>
        <span className="ml-auto font-mono text-[11px] text-base-content/60">
          {plural(row.stats.file_count, "file")}
        </span>
      </header>
      {!collapsed && (
        <div
          style={{
            gridTemplateColumns: COLUMN_WIDTHS.map((width) => `${width}px`).join(" "),
            columnGap: 24,
            padding: 12,
          }}
          className="grid border-t border-base-content/10"
        >
          <div>
            <ColumnLabel>{row.content_label}</ColumnLabel>
            {row.lines.map((line) => (
              <LedgerLine
                key={line.key}
                label={lineTitle(line)}
                onHover={(on) => setHovered(on ? line.key : null)}
                onClick={() => handlers.onOpenLine(row, line)}
                {...stateOf(line.key)}
              >
                <LineBody line={line} />
              </LedgerLine>
            ))}
          </div>
          <div>
            <ColumnLabel>Outputs</ColumnLabel>
            {row.outputs.length === 0 && <EmptyLine>none outside this group</EmptyLine>}
            {row.outputs.map(renderLink)}
            {row.folded_output_count > 0 && (
              <EmptyLine>{`+${row.folded_output_count} in dependencies and stdlib`}</EmptyLine>
            )}
          </div>
          <div>
            <ColumnLabel>Inputs</ColumnLabel>
            {row.inputs.length === 0 && <EmptyLine>none outside this group</EmptyLine>}
            {row.inputs.map(renderLink)}
          </div>
        </div>
      )}
    </div>
  );
}

/* -- Nodes --------------------------------------------------------------------- */

function chipFor(link: GroupLink): ChipData {
  return {
    side: link.side,
    title: link.module,
    subtitle: link.path,
    subtitleIsPath: true,
    tone: link.is_stale ? "stale" : link.is_test ? "test" : "neutral",
    subtle: true,
    markers: linkMarkers(link),
    badge: link.call_count > 1 ? `×${link.call_count}` : undefined,
    jumpToFileId: link.file_id ?? undefined,
    focusKey: link.line_key,
  };
}

/** Inputs, the group card and outputs on one line, like a file in the node view. */
export function GroupNodeRow({ row, handlers }: { row: GroupRow; handlers: GroupRowHandlers }) {
  const [hovered, setHovered] = useState<string | null>(null);
  const status = CHANGE_STYLES[row.status];
  const actions = useMemo<FlowActions>(
    () => ({
      focusFunction: () => {},
      hoverFunction: setHovered,
      jumpToFile: (fileId) => {
        const link = [...row.inputs, ...row.outputs].find((item) => item.file_id === fileId);
        if (link) handlers.onOpenLink(link);
      },
      openTrace: () => {},
      toggle: () => {},
    }),
    [row, handlers],
  );
  const inputs = useMemo(() => row.inputs.map(chipFor), [row]);
  const outputs = useMemo(() => row.outputs.map(chipFor), [row]);

  return (
    <FlowActionsContext.Provider value={actions}>
      <div className="flex items-start">
        <ChipColumn side="input" chips={inputs} focus={hovered} />
        <Connector arrow visible={inputs.length > 0} />
        <div className="shrink-0">
          <Card>
            <div style={{ width: SIZES.cardWidth - 2 }}>
              <CardHeader
                icon={groupIcon(row.group)}
                dotClassName={status.dot}
                title={row.group.label}
                kindName="Group"
                subtitle={groupSubtitle(row)}
                trailing={
                  <span className="font-mono text-[10px] text-base-content/50">
                    {plural(row.stats.file_count, "file")}
                  </span>
                }
              />
              <div className="border-t border-base-content/10 pb-2">
                <div className="flex h-5 items-center px-4 text-[10px] uppercase tracking-wide text-base-content/40">
                  {row.content_label}
                </div>
                {row.lines.map((line) => (
                  <div
                    key={line.key}
                    onClick={() => handlers.onOpenLine(row, line)}
                    onMouseEnter={() => setHovered(line.key)}
                    onMouseLeave={() => setHovered(null)}
                    title={lineTitle(line)}
                    style={{ height: LINE_HEIGHT }}
                    className={[
                      "flex cursor-pointer items-center gap-2 px-4 font-mono text-xs",
                      hovered === line.key ? "bg-base-content/10" : "hover:bg-base-content/5",
                    ].join(" ")}
                  >
                    <LineBody line={line} />
                  </div>
                ))}
              </div>
            </div>
          </Card>
        </div>
        <Connector arrow visible={outputs.length > 0} />
        <ChipColumn side="output" chips={outputs} focus={hovered} />
      </div>
    </FlowActionsContext.Provider>
  );
}

/* -- Rows ---------------------------------------------------------------------- */

export function GroupRowView({
  row,
  renderer,
  handlers,
}: {
  row: GroupRow;
  renderer: Renderer;
  handlers: GroupRowHandlers;
}) {
  return renderer === "ledger" ? (
    <GroupLedgerRow row={row} handlers={handlers} />
  ) : (
    <GroupNodeRow row={row} handlers={handlers} />
  );
}

/** Group rows on the same dotted background the Functions view uses. */
export function GroupRows({
  rows,
  renderer,
  handlers,
  anchor,
}: {
  rows: GroupRow[];
  renderer: Renderer;
  handlers: GroupRowHandlers;
  /** An element id for each row, so a tree can scroll to it. */
  anchor?: (row: GroupRow) => string;
}) {
  return (
    <div className="dot-grid overflow-x-auto">
      <div className={`mx-auto w-max p-8 ${renderer === "ledger" ? "space-y-4" : "space-y-14"}`}>
        {rows.map((row) => (
          <div key={row.group.id} id={anchor?.(row)}>
            <GroupRowView row={row} renderer={renderer} handlers={handlers} />
          </div>
        ))}
      </div>
    </div>
  );
}

/** The existing review for these files. `focusFileId` scrolls to the file that was clicked. */
export function FunctionsView({
  files,
  renderer,
  focusFileId = null,
}: {
  files: ExampleFile[];
  renderer: Renderer;
  focusFileId?: string | null;
}) {
  const container = useRef<HTMLDivElement>(null);
  useEffect(() => {
    if (!focusFileId) return;
    container.current
      ?.querySelector(`#review-file-${focusFileId}`)
      ?.scrollIntoView({ block: "nearest", behavior: "smooth" });
  }, [focusFileId]);
  return (
    <div ref={container}>
      <HtmlReview
        files={files}
        view={renderer === "nodes" ? "chips" : "ledger"}
        outside="folded"
      />
    </div>
  );
}
