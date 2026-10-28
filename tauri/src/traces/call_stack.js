// The call stack as drawn: which frames are on the page after the filters,
// where each one hangs, and what is open. Changed project functions carry
// their diff, unchanged ones only their name, calls outside the project are
// small. Closing a frame hides its code and the frames below it.

/** @typedef {"changed" | "unchanged" | "system"} FrameEmphasis */

export const SHOWN_OPTIONS = [
  { value: "shown", label: "Shown" },
  { value: "hidden", label: "Hidden" },
];

export const STOP_LABELS = {
  cycle: "cycle",
  max_depth: "depth limit",
  max_node_count: "frame limit",
};

/** @returns {FrameEmphasis} */
export function frameEmphasis(frame) {
  if (!frame.defined_in_project) return "system";
  if (frame.change && !frame.repeated) return "changed";
  return "unchanged";
}

export function functionLabel({ module, name, arity }) {
  return `${module}.${name}/${arity}`;
}

/** The description of a test, without the `test ` an Elixir test's name starts with. */
export function testLabel(test) {
  return test.name.replace(/^test /, "");
}

export function sameFunction(a, b) {
  return a.module === b.module && a.name === b.name && a.arity === b.arity;
}

/** Where the call that led to the frame is: `file.ex:11,14`. Null on the test. */
export function calledFrom(frame) {
  if (!frame.call_file_path) return null;
  return `${frame.call_file_path.split("/").pop()}:${frame.call_lines.join(",")}`;
}

/**
 * The indexes of the frames the filters keep. The test itself always stays,
 * so the stack has a top.
 */
export function keptFrameIndexes(frames, { systemCalls, unchanged }) {
  return frames.flatMap((frame, index) => {
    if (index === 0) return [index];
    const emphasis = frameEmphasis(frame);
    if (systemCalls === "hidden" && emphasis === "system") return [];
    if (unchanged === "hidden" && emphasis === "unchanged") return [];
    return [index];
  });
}

/** The index of the frame that called this one: the nearest one above with a smaller depth. */
function callerIndex(frames, index) {
  for (let candidate = index - 1; candidate >= 0; candidate -= 1) {
    if (frames[candidate].depth < frames[index].depth) return candidate;
  }
  return null;
}

/** The frames below each kept frame: the run of deeper frames that follows it. */
function childCounts(frames, kept) {
  const counts = new Map();
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

/** Leaves out every frame below a frame whose children are hidden. */
function withoutHiddenChildren(frames, kept, childrenHiddenIds) {
  let hiddenBelowDepth = null;
  return kept.filter((index) => {
    const frame = frames[index];
    if (hiddenBelowDepth !== null && frame.depth > hiddenBelowDepth) return false;
    hiddenBelowDepth = childrenHiddenIds.has(index) ? frame.depth : null;
    return true;
  });
}

/**
 * One frame as drawn, after the filters and the closed frames are applied.
 * @typedef {object} VisibleFrame
 * @property {number} index position in the backend's frame list, which is also its identity
 * @property {object} frame
 * @property {FrameEmphasis} emphasis
 * @property {number} depth among the visible frames alone, so indents have no gaps
 * @property {number} skippedCallers callers between this frame and the visible one above that the filters left out
 * @property {number} childCount frames below this one that pass the filters, hidden or not
 * @property {boolean} codeClosed the diff of a changed frame is hidden
 * @property {boolean} childrenHidden the frames below are hidden
 */

/** @returns {VisibleFrame[]} */
export function visibleFrames(frames, kept, { codeClosedIds, childrenHiddenIds }) {
  const counts = childCounts(frames, kept);
  const shown = withoutHiddenChildren(frames, kept, childrenHiddenIds);
  const shownSet = new Set(shown);
  const byIndex = new Map();
  return shown.map((index) => {
    let caller = callerIndex(frames, index);
    let skippedCallers = 0;
    while (caller !== null && !shownSet.has(caller)) {
      caller = callerIndex(frames, caller);
      skippedCallers += 1;
    }
    const visible = {
      index,
      frame: frames[index],
      emphasis: frameEmphasis(frames[index]),
      depth: caller === null ? 0 : (byIndex.get(caller)?.depth ?? 0) + 1,
      skippedCallers,
      childCount: counts.get(index) ?? 0,
      codeClosed: codeClosedIds.has(index),
      childrenHidden: childrenHiddenIds.has(index),
    };
    byIndex.set(index, visible);
    return visible;
  });
}

/** True when closing the frame would take something away. */
export function canClose(visible) {
  return visible.childCount > 0 || visible.emphasis === "changed";
}

/** True when nothing of the frame is left to close: no code shown, no frames below. */
export function isClosed(visible) {
  const codeOpen = visible.emphasis === "changed" && !visible.codeClosed;
  const childrenShown = visible.childCount > 0 && !visible.childrenHidden;
  return !codeOpen && !childrenShown;
}
