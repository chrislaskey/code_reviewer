import { useState, type ReactNode } from "react";
import type { Node, NodeProps } from "@xyflow/react";
import {
  ChevronDownIcon,
  ChevronRightIcon,
} from "@heroicons/react/16/solid";
import { useFlowActions } from "./actions";
import {
  calleeLabel,
  callerLabel,
  fileStatus,
  functionKey,
  isOutsideProject,
  shortPath,
  splitCallees,
  type ExampleFile,
  type OutsideMode,
} from "./data";
import {
  LedgerLine,
  LineCounts,
  MarkerIcon,
  calleeMarkers,
  calleeTone,
  callerMarkers,
  callerTone,
  coverageTone,
  plural,
  type Marker,
} from "./nodes";
import { CHANGE_STYLES, TONE_STYLES, type ChipTone } from "./style";

/*
 * The E3 ledger with its options as a style: one node, many iterations.
 * Columns are File / Outputs / Inputs, no edges. Every height is computed
 * here so the page can stack rows without measuring.
 */

export interface LedgerStyle {
  /** Callers on two lines, with file and line under the name. */
  stackedInputs: boolean;
  /** Outputs and inputs as wrapping pills instead of one line each. */
  pills: boolean;
  /** A strip of stale callers and new calls under the header. */
  attention: boolean;
  /** Test coverage in the header. */
  coverage: boolean;
  /** Function, call and caller counts in the header. */
  counts: boolean;
  /** Group outputs and inputs by changed function, or sort them by name. */
  order: "by_function" | "alphabetical";
}

export type LedgerData = {
  file: ExampleFile;
  outside: OutsideMode;
  style: LedgerStyle;
  collapsed: boolean;
  selectedLabel: string | null;
};

export const LEDGER_WIDTH = 1040;
const COLUMN_WIDTHS = [270, 348, 348] as const;
const COLUMN_GAP = 24;

const PART = {
  header: 52,
  label: 20,
  line: 30,
  stackedLine: 40,
  pill: 22,
  pillGap: 6,
  pillPadding: 4,
  padding: 12,
  border: 2,
  divider: 1,
} as const;

/* -- Entries: what one column lists ------------------------------------------ */

interface Entry {
  id: string;
  label: string;
  detail?: string;
  /** Second line under the label, for stacked lines. */
  subdetail?: string;
  tone: ChipTone;
  markers: Marker[];
  dim: boolean;
  /** The changed function this entry belongs to. Null never fades. */
  hoverKey: string | null;
  /** What a click selects. */
  selectionLabel: string;
  fold?: boolean;
}

function sortedByName(entries: Entry[], order: LedgerStyle["order"]): Entry[] {
  if (order === "by_function") return entries;
  const items = entries.filter((entry) => !entry.fold);
  const folds = entries.filter((entry) => entry.fold);
  return [...items.sort((a, b) => a.label.localeCompare(b.label)), ...folds];
}

function outputEntries(file: ExampleFile, outside: OutsideMode, order: LedgerStyle["order"]): Entry[] {
  const { shown, folded } = splitCallees(file.callees, outside);
  const entries: Entry[] = shown.map((callee, index) => ({
    id: `out:${calleeLabel(callee)}:${callee.from}:${index}`,
    label: calleeLabel(callee),
    detail: callee.call_count > 1 ? `×${callee.call_count}` : callee.from,
    tone: calleeTone(callee),
    markers: calleeMarkers(callee),
    dim: isOutsideProject(callee),
    hoverKey: callee.from,
    selectionLabel: calleeLabel(callee),
  }));
  if (folded.length > 0) {
    entries.push({
      id: "out:folded",
      label: `+${folded.length} in dependencies and stdlib`,
      tone: "neutral",
      markers: [],
      dim: true,
      hoverKey: null,
      selectionLabel: "",
      fold: true,
    });
  }
  return sortedByName(entries, order);
}

function inputEntries(file: ExampleFile, order: LedgerStyle["order"]): Entry[] {
  const entries: Entry[] = file.callers.map((caller, index) => ({
    id: `in:${callerLabel(caller)}:${caller.line}:${index}`,
    label: callerLabel(caller),
    detail: caller.calls,
    subdetail: `${shortPath(caller.file_path)}:${caller.line}`,
    tone: callerTone(caller),
    markers: callerMarkers(caller),
    dim: caller.kind === "test",
    hoverKey: caller.calls,
    selectionLabel: callerLabel(caller),
  }));
  return sortedByName(entries, order);
}

/* -- Attention strip ---------------------------------------------------------- */

interface Finding {
  id: string;
  tone: "stale" | "new";
  label: string;
  hoverKey: string;
}

function findingsFor(file: ExampleFile): Finding[] {
  const findings: Finding[] = [];
  for (const caller of file.callers.filter((item) => item.stale)) {
    const target = file.functions.find((fn) => functionKey(fn) === caller.calls);
    const oldName = target?.kind === "renamed" ? target.previous_name : undefined;
    findings.push({
      id: `stale:${callerLabel(caller)}:${caller.line}`,
      tone: "stale",
      label: `${callerLabel(caller)} still calls ${oldName ?? caller.calls}`,
      hoverKey: caller.calls,
    });
  }
  for (const fn of file.functions) {
    const key = functionKey(fn);
    const added = file.callees.filter(
      (callee) => callee.from === key && callee.origin === "project" && callee.diff === "new",
    );
    if (added.length > 0) {
      findings.push({
        id: `new:${key}`,
        tone: "new",
        label: `${key} · ${plural(added.length, "new project call")}`,
        hoverKey: key,
      });
    }
  }
  return findings;
}

/* -- Sizes -------------------------------------------------------------------- */

const MONO_CHAR_WIDTH = 6.7;
const PILL_CHROME = 2 + 16;
const MARKER_WIDTH = 18;

interface PillSize {
  label: string;
  markers: number;
}

/** Monospace text, so wrapping can be worked out without measuring. */
function pillWidth(pill: PillSize, maxWidth: number): number {
  const width = PILL_CHROME + pill.label.length * MONO_CHAR_WIDTH + pill.markers * MARKER_WIDTH;
  return Math.min(Math.ceil(width), maxWidth);
}

function pillRows(pills: PillSize[], maxWidth: number): number {
  if (pills.length === 0) return 0;
  let rows = 1;
  let used = 0;
  for (const pill of pills) {
    const width = pillWidth(pill, maxWidth);
    if (used > 0 && used + PART.pillGap + width > maxWidth) {
      rows += 1;
      used = width;
    } else {
      used += (used > 0 ? PART.pillGap : 0) + width;
    }
  }
  return rows;
}

function pillStackHeight(rows: number): number {
  if (rows === 0) return PART.line;
  return rows * PART.pill + (rows - 1) * PART.pillGap + PART.pillPadding * 2;
}

function pillSizes(entries: Entry[]): PillSize[] {
  return entries.map((entry) => ({ label: pillLabel(entry), markers: entry.markers.length }));
}

function pillLabel(entry: Entry): string {
  return entry.detail?.startsWith("×") ? `${entry.label} ${entry.detail}` : entry.label;
}

const STRIP_WIDTH = LEDGER_WIDTH - PART.border - PART.padding * 2;

function stripHeight(findings: Finding[]): number {
  if (findings.length === 0) return 0;
  const sizes = findings.map((finding) => ({ label: finding.label, markers: 1 }));
  const rows = pillRows(sizes, STRIP_WIDTH);
  return PART.divider + 16 + rows * PART.pill + (rows - 1) * PART.pillGap;
}

function bodyHeights(data: Omit<LedgerData, "collapsed" | "selectedLabel">) {
  const { file, outside, style } = data;
  const outputs = outputEntries(file, outside, style.order);
  const inputs = inputEntries(file, style.order);
  const fileColumn = file.functions.length * PART.line;
  if (style.pills) {
    return {
      file: fileColumn,
      outputs: pillStackHeight(pillRows(pillSizes(outputs), COLUMN_WIDTHS[1])),
      inputs: pillStackHeight(pillRows(pillSizes(inputs), COLUMN_WIDTHS[2])),
    };
  }
  const inputLine = style.stackedInputs ? PART.stackedLine : PART.line;
  return {
    file: fileColumn,
    outputs: Math.max(outputs.length, 1) * PART.line,
    inputs: Math.max(inputs.length, 1) * inputLine,
  };
}

export function ledgerHeight(data: LedgerData): number {
  const strip = data.style.attention ? stripHeight(findingsFor(data.file)) : 0;
  const top = PART.border + PART.header + strip;
  if (data.collapsed || data.file.plain) return top;
  const heights = bodyHeights(data);
  const tallest = Math.max(heights.file, heights.outputs, heights.inputs);
  return top + PART.divider + PART.padding * 2 + PART.label + tallest;
}

/* -- Pieces ------------------------------------------------------------------- */

function Pill({
  entry,
  faded,
  highlighted,
  onHover,
  onClick,
  selected,
}: {
  entry: Entry;
  faded: boolean;
  highlighted: boolean;
  selected: boolean;
  onHover: (hovering: boolean) => void;
  onClick?: () => void;
}) {
  return (
    <span
      onClick={onClick}
      onMouseEnter={() => onHover(true)}
      onMouseLeave={() => onHover(false)}
      title={entry.label}
      style={{ height: PART.pill }}
      className={[
        "nodrag nopan inline-flex max-w-full items-center gap-1 rounded-full border px-2 font-mono text-[11px]",
        entry.fold ? "border-dashed border-base-content/20 text-base-content/50" : TONE_STYLES[entry.tone],
        onClick ? "cursor-pointer" : "cursor-default",
        entry.dim && !entry.fold ? "text-base-content/60" : "",
        entry.tone === "removed" ? "line-through decoration-rose-500/60" : "",
        highlighted ? "bg-base-content/10 ring-1 ring-base-content/30" : "",
        selected ? "ring-1 ring-base-content/50" : "",
        faded ? "opacity-25" : "",
      ].join(" ")}
    >
      <span className="truncate">{pillLabel(entry)}</span>
      {entry.markers.map((marker) => <MarkerIcon key={marker} marker={marker} />)}
    </span>
  );
}

function ColumnLabel({ children }: { children: string }) {
  return (
    <div
      style={{ height: PART.label }}
      className="text-[10px] font-semibold uppercase tracking-wide text-base-content/40"
    >
      {children}
    </div>
  );
}

function EmptyLine({ children }: { children: string }) {
  return (
    <div
      style={{ height: PART.line }}
      className="flex items-center px-2 font-mono text-[10px] text-base-content/40"
    >
      {children}
    </div>
  );
}

function CoverageBadge({ percent }: { percent: number }) {
  const tone = coverageTone(percent);
  return (
    <span
      title="Test coverage (example data)"
      className="flex items-center gap-2 font-mono text-[11px] text-base-content/60"
    >
      Coverage
      <span className="h-1.5 w-12 overflow-hidden rounded-full bg-base-content/10">
        <span
          className={`block h-full rounded-full ${tone.bar}`}
          style={{ width: `${percent}%` }}
        />
      </span>
      <span className={tone.text}>{percent}%</span>
    </span>
  );
}

/**
 * Hover any line, pill or finding to highlight the changed function it belongs
 * to, together with that function's outputs and inputs. Everything else fades.
 */
export function LedgerView({
  id,
  data,
  footer,
}: {
  id: string;
  data: LedgerData;
  /**
   * Shown under the columns while expanded, given the key of the function being hovered.
   * Gives the ledger the height of its content.
   */
  footer?: (hoveredKey: string | null) => ReactNode;
}) {
  const { file, outside, style, collapsed, selectedLabel } = data;
  const plain = file.plain === true;
  const actions = useFlowActions();
  const [hovered, setHovered] = useState<string | null>(null);
  const status = CHANGE_STYLES[fileStatus(file)];
  const Chevron = collapsed ? ChevronRightIcon : ChevronDownIcon;
  const findings = style.attention && !plain ? findingsFor(file) : [];
  const outputs = outputEntries(file, outside, style.order);
  const inputs = inputEntries(file, style.order);

  const stateOf = (hoverKey: string | null) => ({
    faded: hovered !== null && hoverKey !== null && hovered !== hoverKey,
    highlighted: hovered !== null && hovered === hoverKey,
  });
  const hover = (hoverKey: string | null) => (hovering: boolean) =>
    setHovered(hovering ? hoverKey : null);

  const renderEntry = (entry: Entry, stacked: boolean) => {
    const select = entry.fold ? undefined : () => actions.openTrace(entry.selectionLabel);
    const selected = !entry.fold && selectedLabel === entry.selectionLabel;
    if (style.pills) {
      return (
        <Pill
          key={entry.id}
          entry={entry}
          selected={selected}
          onHover={hover(entry.hoverKey)}
          onClick={select}
          {...stateOf(entry.hoverKey)}
        />
      );
    }
    return (
      <LedgerLine
        key={entry.id}
        label={entry.label}
        detail={entry.fold ? undefined : entry.detail}
        markers={entry.markers}
        tone={entry.tone}
        dim={entry.dim}
        height={stacked ? PART.stackedLine : PART.line}
        selected={selected}
        onHover={hover(entry.hoverKey)}
        onClick={select}
        {...stateOf(entry.hoverKey)}
      >
        {stacked ? (
          <span className="min-w-0 flex-1 leading-4">
            <span className="block truncate">{entry.label}</span>
            <span className="block truncate text-[10px] text-base-content/50">
              {entry.subdetail}
            </span>
          </span>
        ) : undefined}
      </LedgerLine>
    );
  };

  const pillColumn = (entries: Entry[], empty: string) =>
    entries.length === 0 ? (
      <EmptyLine>{empty}</EmptyLine>
    ) : (
      <div style={{ padding: `${PART.pillPadding}px 0` }} className="flex flex-wrap gap-1.5">
        {entries.map((entry) => renderEntry(entry, false))}
      </div>
    );

  return (
    <div
      style={{ width: LEDGER_WIDTH, height: footer && !collapsed ? undefined : ledgerHeight(data) }}
      className="overflow-hidden rounded-xl border border-base-300 bg-base-100 shadow-sm"
    >
      <header
        style={{ height: PART.header }}
        onClick={() => actions.toggle(id)}
        className="nodrag nopan flex cursor-pointer items-center gap-3 px-4"
      >
        <Chevron className="size-4 text-base-content/50" />
        <span className={`size-2.5 rounded-full ${status.dot}`} />
        <h3 className="text-sm font-semibold">{file.module}</h3>
        <span className="font-mono text-[11px] text-base-content/50">{file.path}</span>
        <span className="ml-auto flex items-center gap-4 font-mono text-[11px] text-base-content/60">
          {style.counts && !plain && <span>{plural(file.functions.length, "function")}</span>}
          {style.coverage && !plain && file.coverage_percent !== undefined && (
            <CoverageBadge percent={file.coverage_percent} />
          )}
        </span>
      </header>

      {findings.length > 0 && (
        <div
          style={{ height: stripHeight(findings), padding: `8px ${PART.padding}px` }}
          className="flex flex-wrap content-start gap-1.5 border-t border-base-content/10"
        >
          {findings.map((finding) => (
            <span
              key={finding.id}
              onMouseEnter={() => setHovered(finding.hoverKey)}
              onMouseLeave={() => setHovered(null)}
              style={{ height: PART.pill }}
              className={[
                "nodrag nopan inline-flex max-w-full items-center gap-1 rounded-full border px-2 font-mono text-[11px]",
                TONE_STYLES[finding.tone],
                hovered === finding.hoverKey ? "ring-1 ring-base-content/40" : "",
              ].join(" ")}
            >
              <MarkerIcon marker={finding.tone === "stale" ? "stale" : "new"} />
              <span className="truncate">{finding.label}</span>
            </span>
          ))}
        </div>
      )}

      {!collapsed && !plain && (
        <div
          style={{
            padding: PART.padding,
            gridTemplateColumns: COLUMN_WIDTHS.map((width) => `${width}px`).join(" "),
            columnGap: COLUMN_GAP,
          }}
          className="grid border-t border-base-content/10"
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
                  {...stateOf(key)}
                >
                  <span className={`w-3 text-center font-bold ${fnStyle.text}`}>{fnStyle.sign}</span>
                  <span className="min-w-0 flex-1 truncate">
                    {key}
                    {fn.previous_name && (
                      <span className="text-base-content/40"> ← {fn.previous_name}</span>
                    )}
                  </span>
                  <LineCounts fn={fn} className="text-[10px]" />
                </LedgerLine>
              );
            })}
          </div>
          <div>
            <ColumnLabel>Outputs</ColumnLabel>
            {style.pills
              ? pillColumn(outputs, "none")
              : outputs.length === 0
                ? <EmptyLine>none</EmptyLine>
                : outputs.map((entry) => renderEntry(entry, false))}
          </div>
          <div>
            <ColumnLabel>Inputs</ColumnLabel>
            {style.pills
              ? pillColumn(inputs, "none outside this file")
              : inputs.length === 0
                ? <EmptyLine>none outside this file</EmptyLine>
                : inputs.map((entry) => renderEntry(entry, style.stackedInputs))}
          </div>
        </div>
      )}
      {!collapsed && footer?.(hovered)}
    </div>
  );
}

export function LedgerNode({ id, data }: NodeProps<Node<LedgerData, "ledger">>) {
  return <LedgerView id={id} data={data} />;
}
