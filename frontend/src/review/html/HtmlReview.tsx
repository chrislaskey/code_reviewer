import { memo, useMemo, useState } from "react";
import { FlowActionsContext, type FlowActions } from "../../explorations/actions";
import { BlockView, type BlockData } from "../../explorations/block";
import { splitCallees, type ExampleFile, type OutsideMode } from "../../explorations/data";
import { SIZES } from "../../explorations/layout";
import { LedgerView } from "../../explorations/ledger";
import {
  ChipView,
  FileCardView,
  type ChipData,
  type FileCardData,
} from "../../explorations/nodes";
import {
  blockRowsForCallees,
  blockRowsForCallers,
  chipForCallee,
  chipForCaller,
  countItems,
} from "../../explorations/view_data";
import { BLOCKS_LOOK, CHIPS_LOOK, LEDGER_STYLE } from "../looks";
import type { ReviewView } from "../ReviewCanvas";

/*
 * The review as a normal web page: plain HTML and Tailwind, no ReactFlow. Each
 * file is one row of inputs, file card and outputs. The lines are CSS. Hover
 * state lives in the row, so only that row redraws.
 */

/** Distance from the top of a row to the middle of the file card header. */
const LINE_TOP = SIZES.handleTop;
const CONNECTOR_WIDTH = 40;
const STUB_WIDTH = 24;
const GROUP_GAP = 20;
const DIMMED_OPACITY = 0.2;
const LINE_COLOR = "bg-slate-400";

function jumpToFile(fileId: string) {
  document
    .getElementById(`review-file-${fileId}`)
    ?.scrollIntoView({ behavior: "smooth", block: "center" });
}

const NO_ACTIONS: FlowActions = {
  focusFunction: () => {},
  hoverFunction: () => {},
  jumpToFile,
  openTrace: () => {},
  toggle: () => {},
};

function Arrowhead({ className }: { className: string }) {
  return (
    <span
      className={`absolute size-0 border-y-4 border-l-[7px] border-y-transparent border-l-slate-400 ${className}`}
    />
  );
}

/** The line between a column and the file card. */
function Connector({ arrow, visible }: { arrow: boolean; visible: boolean }) {
  return (
    <div
      className={`relative shrink-0 ${visible ? "" : "invisible"}`}
      style={{ width: CONNECTOR_WIDTH, marginTop: LINE_TOP }}
    >
      <span className={`block h-px ${LINE_COLOR}`} />
      {arrow && <Arrowhead className="right-0 top-0 -translate-y-1/2" />}
    </div>
  );
}

/** Chips with a vertical line down the side facing the card and a short line to each chip. */
function ChipColumn({
  side,
  chips,
  focus,
}: {
  side: "input" | "output";
  chips: ChipData[];
  focus: string | null;
}) {
  const half = SIZES.chipHeight / 2;
  const width = SIZES.chipWidth + STUB_WIDTH;
  if (chips.length === 0) return <div className="shrink-0" style={{ width }} />;

  const isInput = side === "input";
  return (
    <div className="relative shrink-0" style={{ width, paddingTop: LINE_TOP - half }}>
      <span
        className={`absolute w-px ${LINE_COLOR} ${isInput ? "right-0" : "left-0"}`}
        style={{ top: LINE_TOP, bottom: half }}
      />
      {chips.map((data, index) => {
        const startsGroup = index > 0 && chips[index - 1].focusKey !== data.focusKey;
        const dimmed = focus !== null && data.focusKey !== focus;
        const stub = (
          <span className={`relative h-px shrink-0 ${LINE_COLOR}`} style={{ width: STUB_WIDTH }}>
            {!isInput && <Arrowhead className="right-0 top-0 -translate-y-1/2" />}
          </span>
        );
        return (
          <div
            key={index}
            className="flex items-center"
            style={{
              marginTop: index === 0 ? 0 : startsGroup ? GROUP_GAP : SIZES.chipGap,
              opacity: dimmed ? DIMMED_OPACITY : 1,
            }}
          >
            {isInput && <ChipView data={data} />}
            {stub}
            {!isInput && <ChipView data={data} />}
          </div>
        );
      })}
    </div>
  );
}

function BlockColumn({ data }: { data: BlockData | null }) {
  if (!data) return <div className="shrink-0" style={{ width: SIZES.blockWidth }} />;
  return <BlockView data={data} />;
}

const NodeRow = memo(function NodeRow({
  files,
  file,
  view,
  outside,
}: {
  files: ExampleFile[];
  file: ExampleFile;
  view: "chips" | "blocks";
  outside: OutsideMode;
}) {
  const [hovered, setHovered] = useState<string | null>(null);
  const [pinned, setPinned] = useState<string | null>(null);
  const focus = hovered ?? pinned;
  const actions = useMemo<FlowActions>(
    () => ({ ...NO_ACTIONS, focusFunction: setPinned, hoverFunction: setHovered }),
    [],
  );

  const chips = useMemo(() => {
    if (view !== "chips") return null;
    const options = { look: CHIPS_LOOK, compact: false };
    return {
      inputs: file.callers.map((caller) => chipForCaller(files, file, caller, options)),
      outputs: splitCallees(file.callees, outside).shown.map((callee) =>
        chipForCallee(files, file, callee, options),
      ),
    };
  }, [view, files, file, outside]);

  const blocks = useMemo(() => {
    if (view !== "blocks") return null;
    return {
      inputs: blockRowsForCallers(files, file, true),
      outputs: blockRowsForCallees(files, file, outside, true),
    };
  }, [view, files, file, outside]);

  const cardData: FileCardData = {
    file,
    ports: false,
    focusKey: focus,
    stats: "none",
    look: view === "chips" ? CHIPS_LOOK : BLOCKS_LOOK,
  };

  const blockData = (side: "input" | "output"): BlockData | null => {
    const rows = side === "input" ? blocks?.inputs : blocks?.outputs;
    if (!rows || rows.length === 0) return null;
    return {
      side,
      title: `${side === "input" ? "Inputs" : "Outputs"} · ${countItems(rows)}`,
      rows,
      focus,
    };
  };

  const hasInputs = (chips?.inputs.length ?? blocks?.inputs.length ?? 0) > 0;
  const hasOutputs = (chips?.outputs.length ?? blocks?.outputs.length ?? 0) > 0;

  return (
    <FlowActionsContext.Provider value={actions}>
      <div id={`review-file-${file.id}`} className="flex items-start">
        {chips ? (
          <ChipColumn side="input" chips={chips.inputs} focus={focus} />
        ) : (
          <BlockColumn data={blockData("input")} />
        )}
        <Connector arrow visible={hasInputs} />
        <div className="shrink-0">
          <FileCardView data={cardData} />
        </div>
        <Connector arrow={view === "blocks"} visible={hasOutputs} />
        {chips ? (
          <ChipColumn side="output" chips={chips.outputs} focus={focus} />
        ) : (
          <BlockColumn data={blockData("output")} />
        )}
      </div>
    </FlowActionsContext.Provider>
  );
});

const LedgerRow = memo(function LedgerRow({
  file,
  outside,
}: {
  file: ExampleFile;
  outside: OutsideMode;
}) {
  const [collapsed, setCollapsed] = useState(false);
  const actions = useMemo<FlowActions>(
    () => ({ ...NO_ACTIONS, toggle: () => setCollapsed((current) => !current) }),
    [],
  );
  const data = useMemo(
    () => ({ file, outside, style: LEDGER_STYLE, collapsed, selectedLabel: null }),
    [file, outside, collapsed],
  );
  return (
    <FlowActionsContext.Provider value={actions}>
      <div id={`review-file-${file.id}`}>
        <LedgerView id={`${file.id}:ledger`} data={data} />
      </div>
    </FlowActionsContext.Provider>
  );
});

/** Every changed file as plain HTML, in one of the three views. */
export function HtmlReview({
  files,
  view,
  outside,
}: {
  files: ExampleFile[];
  view: ReviewView;
  outside: OutsideMode;
}) {
  return (
    <div className="dot-grid overflow-x-auto">
      <div className={`mx-auto w-max p-8 ${view === "ledger" ? "space-y-4" : "space-y-14"}`}>
        {files.map((file) =>
          view === "ledger" ? (
            <LedgerRow key={file.id} file={file} outside={outside} />
          ) : (
            <NodeRow key={file.id} files={files} file={file} view={view} outside={outside} />
          ),
        )}
      </div>
    </div>
  );
}
