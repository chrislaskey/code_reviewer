import { useMemo, useRef, useState } from "react";
import type { CallStackFrame } from "../../api/types";

/*
 * What the HTML and ReactFlow versions share: which frames are on the page,
 * where each one hangs, and what is open. Changed project functions carry
 * their diff, unchanged ones only their name, calls outside the project are
 * small. A click on a frame closes its code and its children together; the
 * toolbar opens or closes the code, or the children, of every frame.
 */

export type FrameEmphasis = "changed" | "unchanged" | "system";

export type ShownMode = "shown" | "hidden";

export const SHOWN_OPTIONS: { value: ShownMode; label: string }[] = [
  { value: "shown", label: "Shown" },
  { value: "hidden", label: "Hidden" },
];

export function frameEmphasis(frame: CallStackFrame): FrameEmphasis {
  if (!frame.defined_in_project) return "system";
  if (frame.change && !frame.repeated) return "changed";
  return "unchanged";
}

/** One frame as drawn, after the filters and the closed frames are applied. */
export interface VisibleFrame {
  /** Position in the backend's frame list, which is also its identity. */
  index: number;
  frame: CallStackFrame;
  emphasis: FrameEmphasis;
  /** Depth among the visible frames alone, so indents have no gaps. */
  depth: number;
  /** The index of the nearest visible frame above on the call path. Null on the test. */
  caller: number | null;
  /** Callers between this frame and `caller` that the filters left out. */
  skippedCallers: number;
  /** Frames below this one that pass the filters, hidden or not. Zero on a leaf. */
  childCount: number;
  /** The diff of a changed frame is hidden. */
  codeClosed: boolean;
  /** The frames below are hidden. */
  childrenHidden: boolean;
}

/** True when closing the frame would take something away. */
export function canClose(frame: VisibleFrame): boolean {
  return frame.childCount > 0 || frame.emphasis === "changed";
}

/** True when nothing of the frame is left to close: no code shown, no frames below. */
export function isClosed(frame: VisibleFrame): boolean {
  const codeOpen = frame.emphasis === "changed" && !frame.codeClosed;
  const childrenShown = frame.childCount > 0 && !frame.childrenHidden;
  return !codeOpen && !childrenShown;
}

interface ClosedState {
  codeClosedIds: Set<number>;
  childrenHiddenIds: Set<number>;
}

/** The index of the frame that called this one: the nearest one above with a smaller depth. */
function callerIndex(frames: CallStackFrame[], index: number): number | null {
  for (let candidate = index - 1; candidate >= 0; candidate -= 1) {
    if (frames[candidate].depth < frames[index].depth) return candidate;
  }
  return null;
}

/** The frames below each kept frame: the run of deeper frames that follows it. */
function childCounts(frames: CallStackFrame[], kept: number[]): Map<number, number> {
  const counts = new Map<number, number>();
  kept.forEach((index, position) => {
    let count = 0;
    for (let next = position + 1; next < kept.length; next += 1) {
      if (frames[kept[next]].depth <= frames[index].depth) break;
      count += 1;
    }
    counts.set(index, count);
  });
  return counts;
}

/** Leave out every frame below a frame whose children are hidden. */
function withoutHiddenChildren(
  frames: CallStackFrame[],
  kept: number[],
  childrenHiddenIds: Set<number>,
): number[] {
  let hiddenBelowDepth: number | null = null;
  return kept.filter((index) => {
    const frame = frames[index];
    if (hiddenBelowDepth !== null && frame.depth > hiddenBelowDepth) return false;
    hiddenBelowDepth = childrenHiddenIds.has(index) ? frame.depth : null;
    return true;
  });
}

function visibleFrames(
  frames: CallStackFrame[],
  kept: number[],
  closed: ClosedState,
): VisibleFrame[] {
  const counts = childCounts(frames, kept);
  const shown = withoutHiddenChildren(frames, kept, closed.childrenHiddenIds);
  const shownSet = new Set(shown);
  const byIndex = new Map<number, VisibleFrame>();
  return shown.map((index) => {
    let caller = callerIndex(frames, index);
    let skippedCallers = 0;
    while (caller !== null && !shownSet.has(caller)) {
      caller = callerIndex(frames, caller);
      skippedCallers += 1;
    }
    const visible: VisibleFrame = {
      index,
      frame: frames[index],
      emphasis: frameEmphasis(frames[index]),
      depth: caller === null ? 0 : (byIndex.get(caller)?.depth ?? 0) + 1,
      caller,
      skippedCallers,
      childCount: counts.get(index) ?? 0,
      codeClosed: closed.codeClosedIds.has(index),
      childrenHidden: closed.childrenHiddenIds.has(index),
    };
    byIndex.set(index, visible);
    return visible;
  });
}

function withIndex(ids: Set<number>, index: number, present: boolean): Set<number> {
  const next = new Set(ids);
  if (present) next.add(index);
  else next.delete(index);
  return next;
}

export interface CallStackViewState {
  visible: VisibleFrame[];
  systemCalls: ShownMode;
  setSystemCalls: (mode: ShownMode) => void;
  unchanged: ShownMode;
  setUnchanged: (mode: ShownMode) => void;
  /** Close the code and the children of a frame, or open both once nothing is left to close. */
  toggleFrame: (index: number) => void;
  setAllCode: (open: boolean) => void;
  setAllChildren: (open: boolean) => void;
}

/** The frames to draw, with the filters and the open state both versions share. */
export function useCallStackView(frames: CallStackFrame[]): CallStackViewState {
  const [systemCalls, setSystemCalls] = useState<ShownMode>("hidden");
  const [unchanged, setUnchanged] = useState<ShownMode>("shown");
  const [closed, setClosed] = useState<ClosedState>({
    codeClosedIds: new Set(),
    childrenHiddenIds: new Set(),
  });

  // The test itself always stays, so the stack has a top.
  const kept = useMemo(
    () =>
      frames.flatMap((frame, index) => {
        if (index === 0) return [index];
        const emphasis = frameEmphasis(frame);
        if (systemCalls === "hidden" && emphasis === "system") return [];
        if (unchanged === "hidden" && emphasis === "unchanged") return [];
        return [index];
      }),
    [frames, systemCalls, unchanged],
  );
  const visible = useMemo(() => visibleFrames(frames, kept, closed), [frames, kept, closed]);

  // Callers made once (ReactFlow actions) read the latest frames through a ref.
  const latestVisible = useRef(visible);
  latestVisible.current = visible;

  function toggleFrame(index: number) {
    const target = latestVisible.current.find((candidate) => candidate.index === index);
    if (!target) return;
    const open = isClosed(target);
    setClosed((current) => ({
      codeClosedIds: withIndex(current.codeClosedIds, index, !open),
      childrenHiddenIds: withIndex(current.childrenHiddenIds, index, !open),
    }));
  }

  const everyIndex = () => new Set(frames.map((_, index) => index));
  function setAllCode(open: boolean) {
    setClosed((current) => ({ ...current, codeClosedIds: open ? new Set() : everyIndex() }));
  }
  function setAllChildren(open: boolean) {
    setClosed((current) => ({ ...current, childrenHiddenIds: open ? new Set() : everyIndex() }));
  }

  return {
    visible,
    systemCalls,
    setSystemCalls,
    unchanged,
    setUnchanged,
    toggleFrame,
    setAllCode,
    setAllChildren,
  };
}
