import { useState, type ReactNode } from "react";
import { ExclamationTriangleIcon } from "@heroicons/react/16/solid";
import {
  EXAMPLE_FILES,
  calleeLabel,
  callerLabel,
  functionKey,
  shortPath,
  type ChangedFunction,
  type ExampleCallee,
  type ExampleCaller,
  type ExampleFile,
} from "../../explorations/data";
import {
  MarkerIcon,
  calleeMarkers,
  calleeTone,
  callerMarkers,
  callerTone,
  plural,
  type Marker,
} from "../../explorations/nodes";
import { ExplorationPage, Legend, Option, Section } from "../../explorations/scaffold";
import { CHANGE_STYLES, TONE_STYLES, type ChipTone } from "../../explorations/style";

/*
 * Ledgers do not need to be flow diagrams. Same hardcoded example, no
 * ReactFlow: plain markup, with columns in the order File / Outputs / Inputs.
 */

const isProject = (callee: ExampleCallee) => callee.origin === "project";

function outputsOf(file: ExampleFile, fn: ChangedFunction): ExampleCallee[] {
  return file.callees.filter((callee) => callee.from === functionKey(fn));
}

function inputsOf(file: ExampleFile, fn: ChangedFunction): ExampleCaller[] {
  return file.callers.filter((caller) => caller.calls === functionKey(fn));
}

/** The one function the pointer is over, as `fileId:name/arity`. */
type HoverState = { hovered: string | null; setHovered: (key: string | null) => void };

function rowClasses(state: HoverState, key: string): string {
  const faded = state.hovered !== null && state.hovered !== key;
  return `${faded ? "opacity-25" : ""}`;
}

function FunctionLabel({ fn }: { fn: ChangedFunction }) {
  const style = CHANGE_STYLES[fn.kind];
  return (
    <div className="flex h-6 min-w-0 items-center gap-2 font-mono text-xs">
      <span className={`w-3 shrink-0 text-center font-bold ${style.text}`} title={style.label}>
        {style.sign}
      </span>
      <span className="min-w-0 flex-1 truncate">
        {functionKey(fn)}
        {fn.previous_name && <span className="text-base-content/40"> ← {fn.previous_name}</span>}
      </span>
      <span className="shrink-0 text-[10px]">
        <span className="text-emerald-600 dark:text-emerald-400">+{fn.added_lines}</span>{" "}
        <span className="text-rose-600 dark:text-rose-400">−{fn.removed_lines}</span>
      </span>
    </div>
  );
}

function FileHeader({ file }: { file: ExampleFile }) {
  const stale = file.callers.filter((caller) => caller.stale).length;
  return (
    <header className="flex items-baseline gap-3 border-b border-base-300 bg-base-200/60 px-4 py-2">
      <h4 className="text-sm font-semibold">{file.module}</h4>
      <span className="font-mono text-[11px] text-base-content/50">{file.path}</span>
      {stale > 0 && (
        <span className="ml-auto flex items-center gap-1 font-mono text-[11px] text-rose-600 dark:text-rose-400">
          <ExclamationTriangleIcon className="size-3.5" />
          {plural(stale, "stale caller")}
        </span>
      )}
    </header>
  );
}

const MARKER_LEGEND = [
  { marker: "stale", label: "stale caller" },
  { marker: "test", label: "test" },
  { marker: "also_changed", label: "also changed" },
  { marker: "new", label: "new call" },
  { marker: "removed", label: "removed call" },
] as const;

function MarkerLegend() {
  return (
    <Legend
      items={MARKER_LEGEND.map(({ marker, label }) => ({
        label,
        swatch: <MarkerIcon marker={marker} />,
      }))}
    />
  );
}

/* -- F1: one row per changed function, calls listed -------------------------- */

function Line({
  label,
  detail,
  tone,
  markers,
  dim,
  stacked,
}: {
  label: string;
  detail?: string;
  /** Put the detail on its own line under the label. */
  stacked?: boolean;
  tone?: ChipTone;
  markers?: Marker[];
  dim?: boolean;
}) {
  return (
    <li
      title={label}
      className={[
        "flex items-center gap-2 font-mono text-[11px]",
        stacked ? "py-1 leading-4" : "leading-6",
        dim ? "text-base-content/55" : "",
        tone === "stale" ? "text-rose-700 dark:text-rose-300" : "",
        tone === "removed" ? "line-through decoration-rose-500/60" : "",
      ].join(" ")}
    >
      {stacked ? (
        <span className="min-w-0 flex-1">
          <span className="block truncate">{label}</span>
          {detail && (
            <span className="block truncate text-[10px] text-base-content/50">{detail}</span>
          )}
        </span>
      ) : (
        <>
          <span className="min-w-0 flex-1 truncate">{label}</span>
          {detail && <span className="shrink-0 text-[10px] text-base-content/50">{detail}</span>}
        </>
      )}
      {markers?.map((marker) => <MarkerIcon key={marker} marker={marker} />)}
    </li>
  );
}

function FunctionRows({
  files,
  state,
  showOutside,
}: {
  files: ExampleFile[];
  state: HoverState;
  showOutside: boolean;
}) {
  return (
    <div className="divide-y divide-base-300 bg-base-100">
      {files.map((file) => (
        <section key={file.id}>
          <FileHeader file={file} />
          <div className="grid grid-cols-[minmax(0,1fr)_minmax(0,1.4fr)_minmax(0,1.4fr)] gap-x-6 px-4 pt-2 text-[10px] font-semibold uppercase tracking-wide text-base-content/40">
            <span>File</span>
            <span>Outputs</span>
            <span>Inputs</span>
          </div>
          {file.functions.map((fn) => {
            const key = `${file.id}:${functionKey(fn)}`;
            const outputs = outputsOf(file, fn);
            const shown = outputs.filter((callee) => showOutside || isProject(callee));
            const hidden = outputs.length - shown.length;
            return (
              <div
                key={key}
                onMouseEnter={() => state.setHovered(key)}
                onMouseLeave={() => state.setHovered(null)}
                className={`grid grid-cols-[minmax(0,1fr)_minmax(0,1.4fr)_minmax(0,1.4fr)] items-start gap-x-6 border-t border-base-300/60 px-4 py-2 first:border-t-0 ${rowClasses(state, key)}`}
              >
                <FunctionLabel fn={fn} />
                <ul>
                  {shown.map((callee, index) => (
                    <Line
                      key={`${calleeLabel(callee)}-${index}`}
                      label={calleeLabel(callee)}
                      detail={callee.call_count > 1 ? `×${callee.call_count}` : undefined}
                      tone={calleeTone(callee)}
                      markers={calleeMarkers(callee)}
                      dim={!isProject(callee)}
                    />
                  ))}
                  {hidden > 0 && (
                    <li className="font-mono text-[10px] leading-6 text-base-content/40">
                      +{hidden} more in dependencies and stdlib
                    </li>
                  )}
                  {outputs.length === 0 && (
                    <li className="font-mono text-[10px] leading-6 text-base-content/40">none</li>
                  )}
                </ul>
                <ul>
                  {inputsOf(file, fn).map((caller, index) => (
                    <Line
                      key={`${callerLabel(caller)}-${index}`}
                      label={callerLabel(caller)}
                      detail={`${shortPath(caller.file_path)}:${caller.line}`}
                      stacked
                      tone={callerTone(caller)}
                      markers={callerMarkers(caller)}
                      dim={caller.kind === "test"}
                    />
                  ))}
                  {inputsOf(file, fn).length === 0 && (
                    <li className="font-mono text-[10px] leading-6 text-base-content/40">
                      none outside this file
                    </li>
                  )}
                </ul>
              </div>
            );
          })}
        </section>
      ))}
    </div>
  );
}

/* -- F2: inline pills --------------------------------------------------------- */

function Pill({
  label,
  tone,
  markers,
  dim,
}: {
  label: string;
  tone: ChipTone;
  markers: Marker[];
  dim?: boolean;
}) {
  return (
    <span
      title={label}
      className={[
        "inline-flex max-w-full items-center gap-1 rounded-full border px-2 py-0.5 font-mono text-[11px]",
        TONE_STYLES[tone],
        dim ? "text-base-content/55" : "",
        tone === "removed" ? "line-through decoration-rose-500/60" : "",
      ].join(" ")}
    >
      <span className="truncate">{label}</span>
      {markers.map((marker) => <MarkerIcon key={marker} marker={marker} />)}
    </span>
  );
}

function PillRows({
  files,
  state,
  showOutside,
}: {
  files: ExampleFile[];
  state: HoverState;
  showOutside: boolean;
}) {
  return (
    <div className="divide-y divide-base-300 bg-base-100">
      {files.map((file) => (
        <section key={file.id}>
          <FileHeader file={file} />
          {file.functions.map((fn) => {
            const key = `${file.id}:${functionKey(fn)}`;
            const outputs = outputsOf(file, fn);
            const shown = outputs.filter((callee) => showOutside || isProject(callee));
            const hidden = outputs.length - shown.length;
            const inputs = inputsOf(file, fn);
            return (
              <div
                key={key}
                onMouseEnter={() => state.setHovered(key)}
                onMouseLeave={() => state.setHovered(null)}
                className={`grid grid-cols-[minmax(0,1fr)_minmax(0,1.4fr)_minmax(0,1.4fr)] items-start gap-x-6 border-t border-base-300/60 px-4 py-2.5 first:border-t-0 ${rowClasses(state, key)}`}
              >
                <FunctionLabel fn={fn} />
                <div className="flex flex-wrap gap-1.5">
                  {shown.map((callee, index) => (
                    <Pill
                      key={`${calleeLabel(callee)}-${index}`}
                      label={`${callee.module}.${functionKey(callee)}`}
                      tone={calleeTone(callee)}
                      markers={calleeMarkers(callee)}
                      dim={!isProject(callee)}
                    />
                  ))}
                  {hidden > 0 && (
                    <span className="px-1 py-0.5 font-mono text-[10px] text-base-content/40">
                      +{hidden} more
                    </span>
                  )}
                </div>
                <div className="flex flex-wrap gap-1.5">
                  {inputs.map((caller, index) => (
                    <Pill
                      key={`${callerLabel(caller)}-${index}`}
                      label={callerLabel(caller)}
                      tone={callerTone(caller)}
                      markers={callerMarkers(caller)}
                    />
                  ))}
                  {inputs.length === 0 && (
                    <span className="px-1 py-0.5 font-mono text-[10px] text-base-content/40">
                      none outside this file
                    </span>
                  )}
                </div>
              </div>
            );
          })}
        </section>
      ))}
    </div>
  );
}

/* -- F3: attention list -------------------------------------------------------- */

interface Finding {
  id: string;
  severity: "needs_attention" | "new_dependency";
  headline: string;
  detail: string;
  file: string;
}

function findFindings(files: ExampleFile[]): Finding[] {
  const findings: Finding[] = [];
  for (const file of files) {
    for (const caller of file.callers.filter((item) => item.stale)) {
      const target = file.functions.find((fn) => functionKey(fn) === caller.calls);
      const renamedFrom = target?.kind === "renamed" ? target.previous_name : undefined;
      findings.push({
        id: `stale:${callerLabel(caller)}:${caller.line}`,
        severity: "needs_attention",
        headline: `${callerLabel(caller)} still calls ${file.module}.${renamedFrom ?? caller.calls}`,
        detail: renamedFrom
          ? `${shortPath(caller.file_path)}:${caller.line}. Renamed to ${caller.calls} in this change.`
          : `${shortPath(caller.file_path)}:${caller.line}. That function was removed in this change.`,
        file: file.module,
      });
    }
    for (const callee of file.callees.filter((item) => isProject(item) && item.diff === "new")) {
      findings.push({
        id: `new:${file.id}:${calleeLabel(callee)}:${callee.from}`,
        severity: "new_dependency",
        headline: `${file.module}.${callee.from} now calls ${calleeLabel(callee)}`,
        detail: `A new call into ${callee.module}.`,
        file: file.module,
      });
    }
  }
  return findings;
}

const SEVERITY_TITLES = {
  needs_attention: "Needs attention",
  new_dependency: "New calls between modules",
} as const;

function AttentionList({ files }: { files: ExampleFile[] }) {
  const findings = findFindings(files);
  return (
    <div className="space-y-5 bg-base-100 p-4">
      {(Object.keys(SEVERITY_TITLES) as Finding["severity"][]).map((severity) => {
        const group = findings.filter((finding) => finding.severity === severity);
        return (
          <section key={severity}>
            <h4 className="mb-2 flex items-center gap-2 text-xs font-semibold uppercase tracking-wide text-base-content/60">
              {severity === "needs_attention" && (
                <ExclamationTriangleIcon className="size-4 text-rose-600 dark:text-rose-400" />
              )}
              {SEVERITY_TITLES[severity]} · {group.length}
            </h4>
            <ul className="divide-y divide-base-300 rounded-lg border border-base-300">
              {group.map((finding) => (
                <li
                  key={finding.id}
                  className={[
                    "flex items-baseline gap-4 px-3 py-2",
                    severity === "needs_attention" ? "bg-rose-500/5" : "",
                  ].join(" ")}
                >
                  <div className="min-w-0 flex-1">
                    <p className="truncate font-mono text-xs font-medium">{finding.headline}</p>
                    <p className="truncate text-[11px] text-base-content/60">{finding.detail}</p>
                  </div>
                  <span className="shrink-0 font-mono text-[10px] text-base-content/40">
                    {finding.file}
                  </span>
                </li>
              ))}
            </ul>
          </section>
        );
      })}
    </div>
  );
}

/* -- Page ----------------------------------------------------------------------- */

function OutsideToggle({
  checked,
  onChange,
  children,
}: {
  checked: boolean;
  onChange: (checked: boolean) => void;
  children: ReactNode;
}) {
  return (
    <label className="flex items-center gap-2 border-t border-base-300 bg-base-100 px-4 py-2 text-[11px] text-base-content/70">
      <input
        type="checkbox"
        checked={checked}
        onChange={(event) => onChange(event.target.checked)}
      />
      {children}
    </label>
  );
}

export default function LedgerVariants() {
  const [hovered, setHovered] = useState<string | null>(null);
  const [showOutside, setShowOutside] = useState(false);
  const state: HoverState = { hovered, setHovered };

  return (
    <ExplorationPage
      title="Ledger variants"
      intro={
        <>
          <p>
            The ledger does not need to be a flow diagram. These are plain markup with no
            ReactFlow, columns in the order File / Outputs / Inputs, using the same hardcoded
            example.
          </p>
          <p>
            Hover a row to fade the others. Dependencies and standard library calls are folded
            away until you turn them on.
          </p>
        </>
      }
    >
      <Section
        id="by-function"
        title="By function"
        intro="One row per changed function, so which calls and callers belong to it is never in doubt and nothing needs drawing."
      >
        <Option
          code="F1"
          title="Function rows, calls listed"
          note="Each changed function with its outputs and inputs listed beside it, one per line, with the file and line of every caller. Easy to read top to bottom. Long when a function has many calls."
        >
          <FunctionRows files={EXAMPLE_FILES} state={state} showOutside={showOutside} />
          <OutsideToggle checked={showOutside} onChange={setShowOutside}>
            Show dependencies and standard library
          </OutsideToggle>
          <MarkerLegend />
        </Option>
        <Option
          code="F2"
          title="Function rows, inline pills"
          note="The same rows with outputs and inputs as wrapping pills. Much shorter, and the pill styling carries the stale, test and new-call signals. Loses the file and line of each caller."
        >
          <PillRows files={EXAMPLE_FILES} state={state} showOutside={showOutside} />
          <OutsideToggle checked={showOutside} onChange={setShowOutside}>
            Show dependencies and standard library
          </OutsideToggle>
          <MarkerLegend />
        </Option>
      </Section>

      <Section
        id="across-files"
        title="Across files"
        intro="Start from what needs a reviewer's eyes instead of from the files."
      >
        <Option
          code="F3"
          title="Attention list"
          note="A flat list across every file: callers of removed or renamed functions first, then new calls between modules. A checklist to read instead of a map to scan. Does not show unchanged context."
        >
          <AttentionList files={EXAMPLE_FILES} />
        </Option>
      </Section>
    </ExplorationPage>
  );
}
