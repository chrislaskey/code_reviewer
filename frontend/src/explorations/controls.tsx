import { useMemo, type ReactNode } from "react";
import type { FlowActions } from "./actions";
import type { FocusNode } from "./canvas";
import type { ExampleFile } from "./data";
import { summarize, summaryLabels } from "./depth";
import { MarkerIcon, type Marker } from "./nodes";
import { Legend } from "./scaffold";

/* Small pieces the depth and variations pages share. */

/** Stand-in for the click-through to a call trace, which is built later. */
export function ClickThroughNotice({ selected, hint }: { selected: string | null; hint: string }) {
  return (
    <p className="border-t border-base-300 bg-base-100 px-4 py-2 font-mono text-[11px] text-base-content/70">
      {selected ? (
        <>
          <span className="font-semibold text-base-content">{selected}</span>
          {" · a call trace from here opens later (not built yet)"}
        </>
      ) : (
        hint
      )}
    </p>
  );
}

const MARKER_LEGEND: { marker: Marker; label: string }[] = [
  { marker: "stale", label: "stale caller" },
  { marker: "test", label: "test" },
  { marker: "also_changed", label: "also changed, click to jump" },
  { marker: "new", label: "new call" },
  { marker: "removed", label: "removed call" },
];

export function MarkerLegend() {
  return (
    <Legend
      items={MARKER_LEGEND.map(({ marker, label }) => ({
        label,
        swatch: <MarkerIcon marker={marker} />,
      }))}
    />
  );
}

export function useActions(overrides: Partial<FlowActions>): FlowActions {
  return useMemo<FlowActions>(
    () => ({
      focusFunction: () => {},
      hoverFunction: () => {},
      jumpToFile: () => {},
      openTrace: () => {},
      toggle: () => {},
      ...overrides,
    }),
    // The overrides are state setters, which never change.
    [],
  );
}

export function jumpRequest(nodeId: string): FocusNode {
  return { id: nodeId, nonce: Date.now() };
}

/** Add the value when missing, remove it when present. */
export function toggled<T>(current: Set<T>, value: T): Set<T> {
  const next = new Set(current);
  if (!next.delete(value)) next.add(value);
  return next;
}

export function SegmentedControl<T extends string>({
  value,
  options,
  onChange,
  label,
}: {
  value: T;
  options: { value: T; label: string }[];
  onChange: (value: T) => void;
  label?: string;
}) {
  return (
    <div className="flex items-center gap-2">
      {label && <span className="text-[11px] text-base-content/60">{label}</span>}
      <div className="inline-flex overflow-hidden rounded-lg border border-base-300 text-xs">
        {options.map((option) => (
          <button
            key={option.value}
            type="button"
            onClick={() => onChange(option.value)}
            className={[
              "px-3 py-1 transition-colors",
              option.value === value
                ? "bg-base-content font-semibold text-base-100"
                : "bg-base-100 text-base-content/60 hover:text-base-content",
            ].join(" ")}
          >
            {option.label}
          </button>
        ))}
      </div>
    </div>
  );
}

export function SummaryStrip({ files, children }: { files: ExampleFile[]; children: ReactNode }) {
  const labels = summaryLabels(summarize(files));
  return (
    <div className="flex flex-wrap items-center gap-x-5 gap-y-2 border-b border-base-300 bg-base-100 px-4 py-2.5">
      <ul className="flex flex-wrap gap-x-5 gap-y-1 font-mono text-xs">
        {labels.map(({ label, alert }) => (
          <li
            key={label}
            className={alert ? "font-semibold text-rose-600 dark:text-rose-400" : "text-base-content/70"}
          >
            {label}
          </li>
        ))}
      </ul>
      <div className="ml-auto">{children}</div>
    </div>
  );
}

/** A bar above a canvas for the options of the variation below it. */
export function Toolbar({ children }: { children: ReactNode }) {
  return (
    <div className="flex flex-wrap items-center gap-x-6 gap-y-2 border-b border-base-300 bg-base-100 px-4 py-2">
      {children}
    </div>
  );
}
