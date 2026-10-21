import { ChevronDownIcon, ChevronRightIcon } from "@heroicons/react/16/solid";
import type { CallStackFrame, TraceStopReason } from "../../api/types";
import { CHANGE_STYLES } from "../../explorations/style";
import { canClose, isClosed, type VisibleFrame } from "./stack_view";

/* The small pieces of a frame that the HTML rows and the ReactFlow nodes share. */

const STOP_LABELS: Record<TraceStopReason, string> = {
  cycle: "cycle",
  max_depth: "depth limit",
  max_node_count: "frame limit",
};

export function functionLabel(frame: CallStackFrame): string {
  return `${frame.function.module}.${frame.function.name}/${frame.function.arity}`;
}

export function calledFrom(frame: CallStackFrame): string | null {
  if (!frame.call_file_path) return null;
  return `${frame.call_file_path.split("/").pop()}:${frame.call_lines.join(",")}`;
}

export function Badge({ children, warn = false }: { children: string; warn?: boolean }) {
  return (
    <span
      className={`shrink-0 rounded border px-1.5 py-px text-[10px] ${
        warn
          ? "border-amber-500/40 text-amber-700 dark:text-amber-300"
          : "border-base-content/15 text-base-content/50"
      }`}
    >
      {children}
    </span>
  );
}

export function FrameBadges({ visible }: { visible: VisibleFrame }) {
  const { frame, skippedCallers } = visible;
  const change = frame.change ? CHANGE_STYLES[frame.change] : null;
  return (
    <>
      {change && !frame.repeated && <Badge warn>{change.label}</Badge>}
      {frame.repeated && <Badge>shown above</Badge>}
      {frame.stop_reason && <Badge>{STOP_LABELS[frame.stop_reason]}</Badge>}
      {skippedCallers > 0 && (
        <Badge>{`via ${skippedCallers} unchanged ${skippedCallers === 1 ? "call" : "calls"}`}</Badge>
      )}
    </>
  );
}

/** Points down while anything of the frame is open. Blank on a frame with nothing to close. */
export function Caret({ visible }: { visible: VisibleFrame }) {
  if (!canClose(visible)) return <span className="size-4 shrink-0" />;
  const Icon = isClosed(visible) ? ChevronRightIcon : ChevronDownIcon;
  return <Icon className="size-4 shrink-0 text-base-content/50" />;
}

export function HiddenCount({ visible }: { visible: VisibleFrame }) {
  if (!visible.childrenHidden || visible.childCount === 0) return null;
  return (
    <span className="shrink-0 rounded-full bg-base-content/10 px-2 py-px text-[10px] text-base-content/60">
      {visible.childCount} hidden below
    </span>
  );
}

export function Trailing({ frame }: { frame: CallStackFrame }) {
  const from = calledFrom(frame);
  return (
    <span className="ml-auto flex min-w-0 shrink items-center gap-3 text-[11px] text-base-content/40">
      {from && <span className="truncate">{from}</span>}
      {frame.file_path && (
        <span className="truncate" title={frame.file_path}>
          {frame.file_path}
        </span>
      )}
    </span>
  );
}

/** The sign and the name of a frame: module muted, function bold. */
export function FrameName({ visible, strong = false }: { visible: VisibleFrame; strong?: boolean }) {
  const { frame } = visible;
  const change = frame.change ? CHANGE_STYLES[frame.change] : null;
  return (
    <>
      <span className={`w-3 shrink-0 text-center font-bold ${change?.text ?? "text-base-content/30"}`}>
        {change?.sign ?? "·"}
      </span>
      {strong ? (
        <span className={`truncate text-sm font-semibold ${change?.name ?? ""}`}>{functionLabel(frame)}</span>
      ) : (
        <span className="truncate">
          <span className="text-base-content/60">{frame.function.module}.</span>
          <span className="font-semibold">{frame.function.name}</span>
          <span className="text-base-content/60">/{frame.function.arity}</span>
        </span>
      )}
    </>
  );
}
