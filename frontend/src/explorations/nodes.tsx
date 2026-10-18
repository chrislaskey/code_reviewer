import { useState, type ReactNode } from "react";
import { Handle, Position, type Node, type NodeProps } from "@xyflow/react";
import {
  BeakerIcon,
  ChevronDownIcon,
  ChevronRightIcon,
  ExclamationTriangleIcon,
  LinkIcon,
  MinusCircleIcon,
  PlusCircleIcon,
} from "@heroicons/react/16/solid";
import {
  ArrowLeftEndOnRectangleIcon,
  ArrowRightStartOnRectangleIcon,
  CodeBracketIcon,
  DocumentTextIcon,
} from "@heroicons/react/20/solid";
import {
  Card,
  CardHeader,
  StartEllipsis,
  StatsRow,
  TabBar,
  type IconComponent,
} from "../components/card";
import { useFlowActions } from "./actions";
import {
  calleeLabel,
  callerLabel,
  fileStatus,
  functionKey,
  shortPath,
  type ChangedFunction,
  type ExampleCallee,
  type ExampleCaller,
  type ExampleFile,
} from "./data";
import { HANDLE_STYLE, SIZES } from "./layout";
import { CHANGE_STYLES, SUBTLE_TONE_STYLES, TONE_STYLES, type ChipTone } from "./style";

/* -- Markers ---------------------------------------------------------------- */

export type Marker = "stale" | "test" | "also_changed" | "new" | "removed";

const MARKERS: Record<Marker, { icon: IconComponent; label: string; className: string }> = {
  stale: {
    icon: ExclamationTriangleIcon,
    label: "Still calls a removed or renamed function",
    className: "text-rose-600 dark:text-rose-400",
  },
  test: { icon: BeakerIcon, label: "Test", className: "text-base-content/50" },
  also_changed: {
    icon: LinkIcon,
    label: "Also changed in this review. Click to jump to it.",
    className: "text-amber-600 dark:text-amber-400",
  },
  new: {
    icon: PlusCircleIcon,
    label: "New call in this change",
    className: "text-emerald-600 dark:text-emerald-400",
  },
  removed: {
    icon: MinusCircleIcon,
    label: "Call removed in this change",
    className: "text-rose-600 dark:text-rose-400",
  },
};

export function MarkerIcon({ marker }: { marker: Marker }) {
  const { icon: Icon, label, className } = MARKERS[marker];
  return (
    <span title={label} className={`shrink-0 ${className}`}>
      <Icon className="size-3.5" />
    </span>
  );
}

export function callerMarkers(caller: ExampleCaller): Marker[] {
  const markers: Marker[] = [];
  if (caller.stale) markers.push("stale");
  if (caller.kind === "test") markers.push("test");
  if (caller.also_changed) markers.push("also_changed");
  return markers;
}

export function callerTone(caller: ExampleCaller): ChipTone {
  if (caller.stale) return "stale";
  return caller.kind === "test" ? "test" : "neutral";
}

export function plural(count: number, noun: string): string {
  return `${count} ${noun}${count === 1 ? "" : "s"}`;
}

export function calleeTone(callee: ExampleCallee): ChipTone {
  if (callee.diff === "new") return "new";
  return callee.diff === "removed" ? "removed" : "neutral";
}

export function calleeMarkers(callee: ExampleCallee): Marker[] {
  if (callee.diff === "new") return ["new"];
  return callee.diff === "removed" ? ["removed"] : [];
}

/** Added and removed line counts. Only the counts the backend knows are shown. */
export function LineCounts({ fn, className }: { fn: ChangedFunction; className?: string }) {
  if (fn.added_lines === undefined && fn.removed_lines === undefined) return null;
  return (
    <span className={className}>
      {fn.added_lines !== undefined && (
        <span className="text-emerald-600 dark:text-emerald-400">+{fn.added_lines}</span>
      )}{" "}
      {fn.removed_lines !== undefined && (
        <span className="text-rose-600 dark:text-rose-400">−{fn.removed_lines}</span>
      )}
    </span>
  );
}

/* -- Chip: one caller or one call ------------------------------------------- */

export type ChipData = {
  side: "input" | "output";
  title: string;
  subtitle?: string;
  tone?: ChipTone;
  markers?: Marker[];
  badge?: string;
  /** Click scrolls to this file's card. */
  jumpToFileId?: string;
  dimOrigin?: boolean;
  /** `fileId:name/arity` of the changed function this chip connects to. */
  focusKey?: string;
  /** One short line, with the detail only in the tooltip. */
  compact?: boolean;
  /** A colour edge instead of a tinted box. */
  subtle?: boolean;
  /** The subtitle is a path: clip the start, keep the file name. */
  subtitleIsPath?: boolean;
};

/** A caller or call as a box. `handle` is the ReactFlow connection point, when there is one. */
export function ChipView({ data, handle }: { data: ChipData; handle?: ReactNode }) {
  const actions = useFlowActions();
  const height = data.compact ? SIZES.compactChipHeight : SIZES.chipHeight;
  const onClick = () =>
    data.jumpToFileId
      ? actions.jumpToFile(data.jumpToFileId)
      : actions.openTrace(data.title);

  return (
    <div
      onClick={onClick}
      onMouseEnter={() => actions.hoverFunction(data.focusKey ?? null)}
      onMouseLeave={() => actions.hoverFunction(null)}
      title={data.compact && data.subtitle ? `${data.title}\n${data.subtitle}` : data.title}
      style={{ height, width: SIZES.chipWidth }}
      className={[
        "flex cursor-pointer items-center gap-2 rounded-lg border px-2.5 shadow-sm transition-colors hover:border-base-content/50",
        (data.subtle ? SUBTLE_TONE_STYLES : TONE_STYLES)[data.tone ?? "neutral"],
        data.tone === "removed" ? "line-through decoration-rose-500/60" : "",
        data.dimOrigin ? "opacity-60" : "",
      ].join(" ")}
    >
      <div className="min-w-0 flex-1">
        <div className="truncate font-mono text-[11px] font-medium">{data.title}</div>
        {data.subtitle && !data.compact && (
          data.subtitleIsPath ? (
            <StartEllipsis className="font-mono text-[10px] text-base-content/50">
              {data.subtitle}
            </StartEllipsis>
          ) : (
            <div className="truncate font-mono text-[10px] text-base-content/50">
              {data.subtitle}
            </div>
          )
        )}
      </div>
      {data.badge && (
        <span className="rounded-full bg-base-content/10 px-1.5 font-mono text-[10px] font-semibold">
          {data.badge}
        </span>
      )}
      {data.markers?.map((marker) => <MarkerIcon key={marker} marker={marker} />)}
      {handle}
    </div>
  );
}

export function ChipNode({ data }: NodeProps<Node<ChipData, "chip">>) {
  return (
    <ChipView
      data={data}
      handle={
        <Handle
          type={data.side === "input" ? "source" : "target"}
          position={data.side === "input" ? Position.Right : Position.Left}
          style={HANDLE_STYLE}
        />
      }
    />
  );
}

/* -- List: many callers or calls in one box --------------------------------- */

export interface ListItem {
  title: string;
  subtitle?: string;
  tone?: ChipTone;
  markers?: Marker[];
}

export type ListData = {
  side: "input" | "output";
  title: string;
  items: ListItem[];
  compact?: boolean;
};

export function ListNode({ data }: NodeProps<Node<ListData, "list">>) {
  const itemHeight = data.compact ? SIZES.listCompactItem : SIZES.listItem;
  return (
    <div
      style={{ width: SIZES.chipWidth }}
      className="rounded-lg border border-base-300 bg-base-100 shadow-sm"
    >
      <div
        style={{ height: SIZES.listHeader }}
        className="flex items-center px-2.5 text-[10px] font-semibold uppercase tracking-wide text-base-content/50"
      >
        {data.title}
      </div>
      <ul style={{ paddingBottom: SIZES.listPadding }}>
        {data.items.map((item, index) => (
          <li
            key={`${item.title}-${index}`}
            style={{ height: itemHeight }}
            title={item.title}
            className={[
              "flex items-center gap-1.5 px-2.5 font-mono text-[11px]",
              item.tone === "removed" ? "line-through decoration-rose-500/60" : "",
            ].join(" ")}
          >
            <span className="min-w-0 flex-1 truncate">{item.title}</span>
            {item.subtitle && (
              <span className="shrink-0 text-[10px] text-base-content/50">
                {item.subtitle}
              </span>
            )}
            {item.markers?.map((marker) => <MarkerIcon key={marker} marker={marker} />)}
          </li>
        ))}
      </ul>
      <Handle
        type={data.side === "input" ? "source" : "target"}
        position={data.side === "input" ? Position.Right : Position.Left}
        style={HANDLE_STYLE}
      />
    </div>
  );
}

/* -- Group: every call of one module in one chip ---------------------------- */

export type GroupData = {
  side: "input" | "output";
  title: string;
  detail: string;
  count: number;
  expanded?: boolean;
  expandable?: boolean;
  tone?: ChipTone;
  markers?: Marker[];
  dimOrigin?: boolean;
};

export function GroupNode({ id, data }: NodeProps<Node<GroupData, "module_group">>) {
  const actions = useFlowActions();
  const Chevron = data.expanded ? ChevronDownIcon : ChevronRightIcon;
  return (
    <div
      onClick={() => (data.expandable ? actions.toggle(id) : actions.openTrace(data.title))}
      title={data.title}
      style={{ height: SIZES.chipHeight, width: SIZES.chipWidth }}
      className={[
        "flex cursor-pointer items-center gap-2 rounded-lg border px-2.5 shadow-sm transition-colors hover:border-base-content/50",
        TONE_STYLES[data.tone ?? "neutral"],
        data.dimOrigin ? "opacity-70" : "",
      ].join(" ")}
    >
      {data.expandable && <Chevron className="size-3.5 shrink-0 text-base-content/50" />}
      <div className="min-w-0 flex-1">
        <div className="truncate font-mono text-[11px] font-semibold">{data.title}</div>
        <div className="truncate font-mono text-[10px] text-base-content/50">{data.detail}</div>
      </div>
      {data.markers?.map((marker) => <MarkerIcon key={marker} marker={marker} />)}
      <span className="rounded-full bg-base-content/10 px-1.5 font-mono text-[10px] font-semibold">
        ×{data.count}
      </span>
      <Handle
        type={data.side === "input" ? "source" : "target"}
        position={data.side === "input" ? Position.Right : Position.Left}
        style={HANDLE_STYLE}
      />
    </div>
  );
}

/* -- File card: the card pattern with the changed functions inside ---------- */

const CARD_PARTS = {
  header: 60,
  stats: 44,
  functionsLabel: 24,
  functionRow: 32,
  functionsPadding: 8,
  tabs: 45,
  border: 2,
} as const;

/** The reading under the card header: counts, test coverage, or nothing. */
export type CardStats = "counts" | "coverage" | "none";

/** How the card looks beyond its stats slot. */
export type CardLook = {
  /** Module name and path only, with a small coverage reading on the title line. */
  pathHeader: boolean;
  /** Colour the function names, not only the sign. */
  coloredFunctions: boolean;
  /** The "Changed functions" label above the list. */
  functionsLabel: boolean;
};

const TOP_PADDING_WITHOUT_LABEL = 8;

export function cardHeight(
  file: ExampleFile,
  withTabs = false,
  stats: CardStats = "counts",
  functionsLabel = true,
): number {
  return (
    CARD_PARTS.header +
    (stats === "none" ? 0 : CARD_PARTS.stats) +
    (functionsLabel ? CARD_PARTS.functionsLabel : TOP_PADDING_WITHOUT_LABEL) +
    file.functions.length * CARD_PARTS.functionRow +
    CARD_PARTS.functionsPadding +
    (withTabs ? CARD_PARTS.tabs : 0) +
    CARD_PARTS.border
  );
}

export type FileCardData = {
  file: ExampleFile;
  /** One handle pair per changed function instead of one per card. */
  ports?: boolean;
  /** `fileId:name/arity` of the highlighted function. */
  focusKey?: string | null;
  /** Show the Source / Trace footer. */
  tabs?: boolean;
  /** Defaults to counts. */
  stats?: CardStats;
  look?: CardLook;
};

const CARD_TABS = [
  { id: "source", label: "Source", icon: CodeBracketIcon },
  { id: "trace_in", label: "Trace in", icon: ArrowLeftEndOnRectangleIcon },
  { id: "trace_out", label: "Trace out", icon: ArrowRightStartOnRectangleIcon },
];

/** Test coverage as a bar. Same height as the counts row. */
export function CoverageRow({ percent }: { percent: number }) {
  const tone = coverageTone(percent);
  return (
    <div className="flex h-[44px] flex-col justify-center gap-1.5 px-4" title="Test coverage (example data)">
      <div className="flex items-baseline justify-between">
        <span className="text-[10px] uppercase tracking-wide text-base-content/40">
          Test coverage
        </span>
        <span className={`font-mono text-sm ${tone.text}`}>{percent}%</span>
      </div>
      <div className="h-1.5 overflow-hidden rounded-full bg-base-content/10">
        <div className={`h-full rounded-full ${tone.bar}`} style={{ width: `${percent}%` }} />
      </div>
    </div>
  );
}

/** The smallest coverage reading: a coloured dot and the percentage. */
export function CoverageDot({ percent }: { percent: number }) {
  const tone = coverageTone(percent);
  return (
    <span
      title="Test coverage (example data)"
      className="flex shrink-0 items-center gap-1 font-mono text-[10px] text-base-content/60"
    >
      <span className={`size-1.5 rounded-full ${tone.bar}`} />
      {percent}%
    </span>
  );
}

export function coverageTone(percent: number): { text: string; bar: string } {
  if (percent < 60) {
    return { text: "text-rose-600 dark:text-rose-400", bar: "bg-rose-500" };
  }
  if (percent < 80) {
    return { text: "text-amber-600 dark:text-amber-400", bar: "bg-amber-400" };
  }
  return { text: "text-emerald-600 dark:text-emerald-400", bar: "bg-emerald-500" };
}

/** The card has a border the other nodes do not, so its handles sit one pixel higher. */
const CARD_HANDLE_STYLE = { top: SIZES.handleTop - 1 } as const;

/** The file card. `handles` are the ReactFlow connection points, when there are any. */
export function FileCardView({ data, handles }: { data: FileCardData; handles?: ReactNode }) {
  const { file, ports, focusKey, tabs, stats = "counts", look } = data;
  const actions = useFlowActions();
  const status = CHANGE_STYLES[fileStatus(file)];

  return (
    <Card>
      <div style={{ width: SIZES.cardWidth - CARD_PARTS.border }}>
        {look?.pathHeader ? (
          <CardHeader
            icon={DocumentTextIcon}
            dotClassName={status.dot}
            title={file.module}
            subtitle={file.path}
            subtitleClippedAtStart
            trailing={
              file.coverage_percent === undefined ? undefined : (
                <CoverageDot percent={file.coverage_percent} />
              )
            }
          />
        ) : (
          <CardHeader
            icon={DocumentTextIcon}
            dotClassName={status.dot}
            title={file.module}
            kindName="File"
            subtitle={shortPath(file.path)}
          />
        )}
        {stats === "counts" && (
          <StatsRow
            stats={[
              { label: "Functions", value: file.functions.length },
              { label: "Callers", value: file.callers.length },
              { label: "Calls out", value: file.callees.length },
            ]}
          />
        )}
        {stats === "coverage" && file.coverage_percent !== undefined && (
          <CoverageRow percent={file.coverage_percent} />
        )}
        <div className="border-t border-base-content/10 pb-2">
          {look?.functionsLabel === false ? (
            <div style={{ height: TOP_PADDING_WITHOUT_LABEL }} />
          ) : (
            <div
              style={{ height: CARD_PARTS.functionsLabel }}
              className="flex items-center px-4 text-[10px] uppercase tracking-wide text-base-content/40"
            >
              Changed functions
            </div>
          )}
          {file.functions.map((fn) => {
            const key = functionKey(fn);
            const fnStyle = CHANGE_STYLES[fn.kind];
            const selected = focusKey === `${file.id}:${key}`;
            return (
              <div
                key={key}
                onClick={() => actions.focusFunction(selected ? null : `${file.id}:${key}`)}
                onMouseEnter={() => actions.hoverFunction(`${file.id}:${key}`)}
                onMouseLeave={() => actions.hoverFunction(null)}
                style={{ height: CARD_PARTS.functionRow }}
                className={[
                  "nodrag nopan relative flex cursor-pointer items-center gap-2 px-4 transition-colors",
                  selected ? "bg-base-content/10" : "hover:bg-base-content/5",
                ].join(" ")}
                title={fnStyle.label}
              >
                <span className={`w-3 text-center font-mono text-xs font-bold ${fnStyle.text}`}>
                  {fnStyle.sign}
                </span>
                <span
                  className={`min-w-0 flex-1 truncate font-mono text-xs ${look?.coloredFunctions ? fnStyle.name : ""}`}
                >
                  {key}
                  {fn.previous_name && (
                    <span className="text-base-content/40"> ← {fn.previous_name}</span>
                  )}
                </span>
                <LineCounts fn={fn} className="font-mono text-[10px]" />
                {ports && (
                  <>
                    <Handle id={`in:${key}`} type="target" position={Position.Left} />
                    <Handle id={`out:${key}`} type="source" position={Position.Right} />
                  </>
                )}
              </div>
            );
          })}
        </div>
        {tabs && (
          <TabBar
            tabs={CARD_TABS}
            active={null}
            onSelect={(id) => actions.openTrace(`${file.module} · ${id}`)}
          />
        )}
      </div>
      {handles}
    </Card>
  );
}

export function FileCardNode({ data }: NodeProps<Node<FileCardData, "file_card">>) {
  return (
    <FileCardView
      data={data}
      handles={
        data.ports ? undefined : (
          <>
            <Handle type="target" position={Position.Left} style={CARD_HANDLE_STYLE} />
            <Handle type="source" position={Position.Right} style={CARD_HANDLE_STYLE} />
          </>
        )
      }
    />
  );
}

/* -- Row: inputs, file and outputs as one ledger line ----------------------- */

export const ROW_PARTS = { header: 52, label: 20, line: 30, padding: 12, border: 2 } as const;
export const ROW_WIDTH = 1040;

export function rowHeight(file: ExampleFile, collapsed = false): number {
  if (collapsed) return ROW_PARTS.header + ROW_PARTS.border;
  const lines = Math.max(file.callers.length, file.functions.length, file.callees.length);
  return (
    ROW_PARTS.header +
    ROW_PARTS.padding * 2 +
    ROW_PARTS.label +
    lines * ROW_PARTS.line +
    ROW_PARTS.border
  );
}

export type RowData = {
  file: ExampleFile;
  /** Marker icons, diff colours, dimmed dependencies and a collapse toggle. */
  refined?: boolean;
  collapsed?: boolean;
  selectedLabel?: string | null;
};

function ColumnLabel({ children }: { children: string }) {
  return (
    <div
      style={{ height: ROW_PARTS.label }}
      className="text-[10px] font-semibold uppercase tracking-wide text-base-content/40"
    >
      {children}
    </div>
  );
}

export function LedgerLine({
  label,
  detail,
  markers = [],
  tone = "neutral",
  dim = false,
  faded = false,
  highlighted = false,
  selected = false,
  height = ROW_PARTS.line,
  onHover,
  onClick,
  children,
}: {
  label: string;
  detail?: string;
  markers?: Marker[];
  tone?: ChipTone;
  /** Quieter text, for calls that rarely need review. */
  dim?: boolean;
  /** Not related to what the pointer is over. */
  faded?: boolean;
  /** Related to what the pointer is over. */
  highlighted?: boolean;
  /** The line last clicked. */
  selected?: boolean;
  height?: number;
  onHover: (hovering: boolean) => void;
  onClick?: () => void;
  children?: ReactNode;
}) {
  return (
    <div
      onClick={onClick}
      onMouseEnter={() => onHover(true)}
      onMouseLeave={() => onHover(false)}
      title={label}
      style={{ height }}
      className={[
        "nodrag nopan flex items-center gap-2 rounded-md px-2 font-mono text-[11px] transition-[opacity,background-color,box-shadow] duration-150",
        onClick ? "cursor-pointer" : "cursor-default",
        highlighted || selected ? "bg-base-content/10" : "hover:bg-base-content/5",
        selected ? "ring-1 ring-base-content/30" : "",
        faded ? "opacity-25" : "",
        dim ? "text-base-content/60" : "",
        tone === "stale" ? "text-rose-700 dark:text-rose-300" : "",
        tone === "removed" ? "line-through decoration-rose-500/60" : "",
      ].join(" ")}
    >
      {children ?? <span className="min-w-0 flex-1 truncate">{label}</span>}
      {detail && <span className="shrink-0 text-[10px] text-base-content/50">{detail}</span>}
      {markers.map((marker) => <MarkerIcon key={marker} marker={marker} />)}
    </div>
  );
}

/**
 * Hover any line to highlight the changed function it belongs to, together
 * with that function's outputs and inputs. Everything else fades.
 */
export function RowNode({ id, data }: NodeProps<Node<RowData, "row">>) {
  const { file, refined, collapsed, selectedLabel } = data;
  const actions = useFlowActions();
  const [hovered, setHovered] = useState<string | null>(null);
  const status = CHANGE_STYLES[fileStatus(file)];
  const Chevron = collapsed ? ChevronRightIcon : ChevronDownIcon;
  const staleCount = file.callers.filter((caller) => caller.stale).length;

  const lineState = (key: string) => ({
    faded: hovered !== null && hovered !== key,
    highlighted: hovered === key,
  });
  const hover = (key: string) => (hovering: boolean) => setHovered(hovering ? key : null);

  return (
    <div
      style={{ width: ROW_WIDTH, height: rowHeight(file, collapsed) }}
      className="overflow-hidden rounded-xl border border-base-300 bg-base-100 shadow-sm"
    >
      <header
        style={{ height: ROW_PARTS.header }}
        onClick={() => refined && actions.toggle(id)}
        className={`flex items-center gap-3 px-4 ${refined ? "nodrag nopan cursor-pointer" : ""}`}
      >
        {refined && <Chevron className="size-4 text-base-content/50" />}
        <span className={`size-2.5 rounded-full ${status.dot}`} />
        <h3 className="text-sm font-semibold">{file.module}</h3>
        <span className="font-mono text-[11px] text-base-content/50">{file.path}</span>
        <span className="ml-auto flex gap-4 font-mono text-[11px] text-base-content/60">
          <span>{plural(file.functions.length, "changed function")}</span>
          <span>{plural(file.callees.length, "call")} out</span>
          <span>{plural(file.callers.length, "caller")}</span>
          {refined && staleCount > 0 && (
            <span className="flex items-center gap-1 text-rose-600 dark:text-rose-400">
              <ExclamationTriangleIcon className="size-3.5" />
              {staleCount} stale
            </span>
          )}
        </span>
      </header>
      {!collapsed && (
        <div
          style={{ padding: ROW_PARTS.padding }}
          className="grid grid-cols-[minmax(0,1fr)_minmax(0,1fr)_minmax(0,1fr)] gap-x-6 border-t border-base-content/10"
        >
          <div>
            <ColumnLabel>File</ColumnLabel>
            {file.functions.map((fn) => {
              const key = functionKey(fn);
              const fnStyle = CHANGE_STYLES[fn.kind];
              return (
                <LedgerLine
                  key={key}
                  label={fnStyle.label}
                  onHover={hover(key)}
                  {...lineState(key)}
                >
                  <span className={`w-3 text-center font-bold ${fnStyle.text}`}>{fnStyle.sign}</span>
                  <span className="min-w-0 flex-1 truncate">
                    {key}
                    {fn.previous_name && (
                      <span className="text-base-content/40"> ← {fn.previous_name}</span>
                    )}
                  </span>
                  <LineCounts fn={fn} className="font-mono text-[10px]" />
                </LedgerLine>
              );
            })}
          </div>
          <div>
            <ColumnLabel>Outputs</ColumnLabel>
            {file.callees.map((callee, index) => (
              <LedgerLine
                key={`${calleeLabel(callee)}-${callee.from}-${index}`}
                label={calleeLabel(callee)}
                detail={callee.call_count > 1 ? `×${callee.call_count}` : callee.from}
                markers={refined ? calleeMarkers(callee) : []}
                tone={refined ? calleeTone(callee) : "neutral"}
                dim={refined && callee.origin !== "project"}
                selected={selectedLabel === calleeLabel(callee)}
                onHover={hover(callee.from)}
                onClick={() => actions.openTrace(calleeLabel(callee))}
                {...lineState(callee.from)}
              />
            ))}
          </div>
          <div>
            <ColumnLabel>Inputs</ColumnLabel>
            {file.callers.map((caller, index) => (
              <LedgerLine
                key={`${callerLabel(caller)}-${caller.line}-${index}`}
                label={callerLabel(caller)}
                detail={caller.calls}
                markers={refined ? callerMarkers(caller) : []}
                tone={refined ? callerTone(caller) : "neutral"}
                dim={refined && caller.kind === "test"}
                selected={selectedLabel === callerLabel(caller)}
                onHover={hover(caller.calls)}
                onClick={() => actions.openTrace(callerLabel(caller))}
                {...lineState(caller.calls)}
              />
            ))}
          </div>
        </div>
      )}
    </div>
  );
}
