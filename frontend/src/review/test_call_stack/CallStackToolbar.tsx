import type { ReactNode } from "react";
import type { GetTestCallStackResponse } from "../../api/types";
import { SegmentedControl, Toolbar } from "../../explorations/controls";
import { frameEmphasis, SHOWN_OPTIONS, type CallStackViewState } from "./stack_view";

/* The controls above the stack, the same in both versions. */

function OpenClosePair({ label, onChange }: { label: string; onChange: (open: boolean) => void }) {
  const button = "px-3 py-1 bg-base-100 text-base-content/60 hover:text-base-content";
  return (
    <div className="inline-flex overflow-hidden rounded-lg border border-base-300 text-xs">
      <button type="button" onClick={() => onChange(true)} className={button}>
        Open {label}
      </button>
      <button type="button" onClick={() => onChange(false)} className={`${button} border-l border-base-300`}>
        Close {label}
      </button>
    </div>
  );
}

export function CallStackToolbar({
  stack,
  view,
  children,
}: {
  stack: GetTestCallStackResponse;
  view: CallStackViewState;
  /** Extra controls after the shared ones. */
  children?: ReactNode;
}) {
  const frames = stack.frames;
  const changedCount = frames.filter((frame) => frameEmphasis(frame) === "changed").length;
  const systemCount = frames.filter((frame) => frameEmphasis(frame) === "system").length;
  const projectCount = frames.length - systemCount;
  return (
    <Toolbar>
      <SegmentedControl
        label="System calls"
        value={view.systemCalls}
        onChange={view.setSystemCalls}
        options={SHOWN_OPTIONS}
      />
      <SegmentedControl
        label="Unchanged functions"
        value={view.unchanged}
        onChange={view.setUnchanged}
        options={SHOWN_OPTIONS}
      />
      <OpenClosePair label="code" onChange={view.setAllCode} />
      <OpenClosePair label="children" onChange={view.setAllChildren} />
      {children}
      <span className="ml-auto font-mono text-[11px] text-base-content/50">
        {changedCount} changed · {projectCount} project {projectCount === 1 ? "function" : "functions"} ·{" "}
        {systemCount} system {systemCount === 1 ? "call" : "calls"}
        {stack.truncated && " · cut short by the limits"}
      </span>
    </Toolbar>
  );
}
