import { useEffect, useMemo, useState } from "react";
import type { GetTestCallStackResponse } from "../../api/types";
import { CodeLines } from "../html/CodeFooter";
import { ELIXIR } from "../html/syntax_highlight";
import { CallStackToolbar } from "./CallStackToolbar";
import { calledFrom, Caret, FrameBadges, FrameName, functionLabel, HiddenCount, Trailing } from "./frame_parts";
import { canClose, useCallStackView, type VisibleFrame } from "./stack_view";

/*
 * The calls below a test as plain HTML, top to bottom like a stack trace. The
 * same three weights as the canvas: changed functions with their diff,
 * unchanged project functions by name, calls outside the project as slim rows.
 */

const INDENT = 24;

function frameElementId(index: number): string {
  return `call-stack-frame-${index}`;
}

/** Thin guide lines, one for each level above this frame. */
function DepthGuides({ depth }: { depth: number }) {
  return (
    <div className="flex shrink-0 self-stretch">
      {Array.from({ length: depth }, (_, level) => (
        <span key={level} className="border-l border-base-content/10" style={{ width: INDENT }} />
      ))}
    </div>
  );
}

function SystemRow({ visible }: { visible: VisibleFrame }) {
  const { frame } = visible;
  const from = calledFrom(frame);
  return (
    <div id={frameElementId(visible.index)} className="flex">
      <DepthGuides depth={visible.depth} />
      <div className="flex min-w-0 items-center gap-2 py-0.5 pl-2 font-mono text-[11px] text-base-content/50">
        <span className="size-4 shrink-0" />
        <span className="w-3 text-center">·</span>
        <span className="truncate">{functionLabel(frame)}</span>
        <span className="shrink-0 rounded border border-base-content/15 px-1.5 py-px text-[10px]">system</span>
        {from && <span className="truncate text-base-content/40">{from}</span>}
      </div>
    </div>
  );
}

function UnchangedRow({ visible, onToggle }: { visible: VisibleFrame; onToggle: () => void }) {
  const closable = canClose(visible);
  return (
    <div id={frameElementId(visible.index)} className="flex">
      <DepthGuides depth={visible.depth} />
      <div
        onClick={closable ? onToggle : undefined}
        className={`my-0.5 flex h-8 min-w-0 flex-1 items-center gap-2 rounded-md border border-base-300 bg-base-100 px-2 font-mono text-xs ${
          closable ? "cursor-pointer" : ""
        }`}
      >
        <Caret visible={visible} />
        <FrameName visible={visible} />
        <FrameBadges visible={visible} />
        <HiddenCount visible={visible} />
        <Trailing frame={visible.frame} />
      </div>
    </div>
  );
}

function ChangedRow({
  visible,
  current,
  onToggle,
}: {
  visible: VisibleFrame;
  current: boolean;
  onToggle: () => void;
}) {
  const { frame } = visible;
  const bar = frame.change === "added" ? "border-l-emerald-500" : "border-l-amber-400";
  return (
    <div id={frameElementId(visible.index)} className="flex">
      <DepthGuides depth={visible.depth} />
      <div
        className={`my-0.5 min-w-0 flex-1 rounded-r-md border border-l-2 border-base-300 bg-base-100 ${bar} ${
          current ? "ring-2 ring-primary/60" : ""
        }`}
      >
        <button
          type="button"
          onClick={onToggle}
          className="flex h-8 w-full cursor-pointer items-center gap-2 px-2 text-left font-mono text-xs"
        >
          <Caret visible={visible} />
          <FrameName visible={visible} strong />
          <FrameBadges visible={visible} />
          <HiddenCount visible={visible} />
          <Trailing frame={frame} />
        </button>
        {!visible.codeClosed && <CodeLines lines={frame.lines} language={ELIXIR} />}
      </div>
    </div>
  );
}

function scrollToFrame(index: number) {
  document.getElementById(frameElementId(index))?.scrollIntoView({ behavior: "smooth", block: "center" });
}

export function CallStackView({ stack }: { stack: GetTestCallStackResponse }) {
  const view = useCallStackView(stack.frames);
  const [currentChange, setCurrentChange] = useState<number | null>(null);
  useEffect(() => setCurrentChange(null), [stack]);

  const changedIndexes = useMemo(
    () => view.visible.flatMap((item) => (item.emphasis === "changed" ? [item.index] : [])),
    [view.visible],
  );

  function stepChange(direction: 1 | -1) {
    if (changedIndexes.length === 0) return;
    const position = currentChange === null ? -1 : changedIndexes.indexOf(currentChange);
    const next = (position + direction + changedIndexes.length) % changedIndexes.length;
    setCurrentChange(changedIndexes[next]);
    scrollToFrame(changedIndexes[next]);
  }

  const stepButton = "px-2 py-1 text-base-content/60 hover:text-base-content disabled:opacity-40";
  const noChanges = changedIndexes.length === 0;

  return (
    <div className="flex min-h-0 flex-1 flex-col">
      <CallStackToolbar stack={stack} view={view}>
        <span className="inline-flex overflow-hidden rounded-lg border border-base-300 text-xs">
          <button type="button" className={stepButton} disabled={noChanges} onClick={() => stepChange(-1)}>
            ↑ Prev change
          </button>
          <button
            type="button"
            className={`${stepButton} border-l border-base-300`}
            disabled={noChanges}
            onClick={() => stepChange(1)}
          >
            Next change ↓
          </button>
        </span>
      </CallStackToolbar>
      <div className="dot-grid min-h-0 flex-1 overflow-auto p-6">
        <div className="mx-auto max-w-6xl">
          {view.visible.map((item) => {
            const onToggle = () => view.toggleFrame(item.index);
            if (item.emphasis === "system") return <SystemRow key={item.index} visible={item} />;
            if (item.emphasis === "unchanged") {
              return <UnchangedRow key={item.index} visible={item} onToggle={onToggle} />;
            }
            return (
              <ChangedRow
                key={item.index}
                visible={item}
                current={currentChange === item.index}
                onToggle={onToggle}
              />
            );
          })}
        </div>
      </div>
    </div>
  );
}
