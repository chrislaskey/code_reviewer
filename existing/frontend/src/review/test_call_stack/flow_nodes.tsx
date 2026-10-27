import { Handle, Position, type Node, type NodeProps } from "@xyflow/react";
import type { CallStackFrame } from "../../api/types";
import { useFlowActions } from "../../explorations/actions";
import { CodeLines, hasGapBetween } from "../html/CodeFooter";
import { ELIXIR } from "../html/syntax_highlight";
import { calledFrom, Caret, FrameBadges, FrameName, functionLabel, HiddenCount, Trailing } from "./frame_parts";
import { canClose, type FrameEmphasis, type VisibleFrame } from "./stack_view";

/*
 * One frame of a test call stack as a ReactFlow node. Three weights: a changed
 * project function is a card with its whole diff; an unchanged project function
 * is one line with its name; a call outside the project is a small muted pill.
 * Every height is computed here so the canvas can stack frames without measuring.
 */

export interface CallStackFrameData extends VisibleFrame, Record<string, unknown> {
  width: number;
}

export const FRAME_PART = {
  changedHeader: 44,
  unchangedHeight: 34,
  systemHeight: 24,
  codeLine: 20,
  codeBottomPadding: 8,
  border: 2,
} as const;

/** Where the edge into a frame meets it: the middle of its header. */
export function frameHandleTop(emphasis: FrameEmphasis): number {
  switch (emphasis) {
    case "changed":
      return FRAME_PART.changedHeader / 2;
    case "unchanged":
      return FRAME_PART.unchangedHeight / 2;
    case "system":
      return FRAME_PART.systemHeight / 2;
  }
}

/** The same as `CodeLines` draws: one row per line, one more per gap, and the bottom padding. */
function codeHeight(frame: CallStackFrame): number {
  const rows = frame.lines.reduce(
    (count, line, index) => count + 1 + (hasGapBetween(frame.lines[index - 1], line) ? 1 : 0),
    0,
  );
  return rows * FRAME_PART.codeLine + FRAME_PART.codeBottomPadding;
}

export function frameHeight(data: CallStackFrameData): number {
  switch (data.emphasis) {
    case "changed":
      return (
        FRAME_PART.border +
        FRAME_PART.changedHeader +
        (data.codeClosed ? 0 : 1 + codeHeight(data.frame))
      );
    case "unchanged":
      return FRAME_PART.unchangedHeight;
    case "system":
      return FRAME_PART.systemHeight;
  }
}

function ChangedFrame({ id, data }: { id: string; data: CallStackFrameData }) {
  const { frame, codeClosed, width } = data;
  const actions = useFlowActions();
  const bar = frame.change === "added" ? "border-l-emerald-500" : "border-l-amber-400";
  return (
    <div
      style={{ width, height: frameHeight(data) }}
      className={`overflow-hidden rounded-xl border border-l-4 border-base-300 bg-base-100 shadow-md ${bar}`}
    >
      <header
        style={{ height: FRAME_PART.changedHeader }}
        onClick={() => actions.toggle(id)}
        className="nodrag nopan flex cursor-pointer items-center gap-2 px-3 font-mono text-xs"
      >
        <Caret visible={data} />
        <FrameName visible={data} strong />
        <FrameBadges visible={data} />
        <HiddenCount visible={data} />
        <Trailing frame={frame} />
      </header>
      {!codeClosed && (
        <div className="nowheel border-t border-base-content/10">
          <CodeLines lines={frame.lines} language={ELIXIR} />
        </div>
      )}
    </div>
  );
}

function UnchangedFrame({ id, data }: { id: string; data: CallStackFrameData }) {
  const { frame, width } = data;
  const actions = useFlowActions();
  return (
    <div
      style={{ width, height: FRAME_PART.unchangedHeight }}
      onClick={() => canClose(data) && actions.toggle(id)}
      className={`nodrag nopan flex items-center gap-2 rounded-lg border border-base-300 bg-base-100 px-3 font-mono text-xs shadow-sm ${
        canClose(data) ? "cursor-pointer" : ""
      }`}
    >
      <Caret visible={data} />
      <FrameName visible={data} />
      <FrameBadges visible={data} />
      <HiddenCount visible={data} />
      <Trailing frame={frame} />
    </div>
  );
}

function SystemFrame({ data }: { data: CallStackFrameData }) {
  const { frame } = data;
  const from = calledFrom(frame);
  return (
    <div
      style={{ height: FRAME_PART.systemHeight }}
      className="inline-flex max-w-full items-center gap-2 rounded-full border border-dashed border-base-content/20 bg-base-100/60 px-2.5 font-mono text-[10px] text-base-content/50"
    >
      <span className="truncate">{functionLabel(frame)}</span>
      {from && <span className="shrink-0 text-base-content/35">{from}</span>}
    </div>
  );
}

/** The handles sit on the left: calls come in from the frame above, and go out below. */
export const SOURCE_HANDLE_LEFT = 14;

export function CallStackFrameNode({ id, data }: NodeProps<Node<CallStackFrameData, "call_stack_frame">>) {
  const handleTop = frameHandleTop(data.emphasis);
  return (
    <div className="relative">
      <Handle type="target" position={Position.Left} style={{ top: handleTop, opacity: 0 }} />
      {data.emphasis === "changed" && <ChangedFrame id={id} data={data} />}
      {data.emphasis === "unchanged" && <UnchangedFrame id={id} data={data} />}
      {data.emphasis === "system" && <SystemFrame data={data} />}
      <Handle
        type="source"
        position={Position.Bottom}
        style={{ left: SOURCE_HANDLE_LEFT, opacity: 0 }}
      />
    </div>
  );
}
