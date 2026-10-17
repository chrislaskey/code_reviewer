import { useMemo, useState } from "react";
import { FlowActionsContext } from "../../explorations/actions";
import { FlowCanvas, type FocusNode } from "../../explorations/canvas";
import { EXAMPLE_FILES, type ExampleFile } from "../../explorations/data";
import {
  buildGroupedExpandable,
  buildLedgerRefined,
  buildReviewCanvas,
  type LedgerSort,
} from "../../explorations/depth";
import { canvasHeight } from "../../explorations/layout";
import {
  ClickThroughNotice,
  MarkerLegend,
  SegmentedControl,
  SummaryStrip,
  jumpRequest,
  useActions,
} from "../../explorations/controls";
import { ExplorationPage, Option, Section } from "../../explorations/scaffold";

const ZOOM = 0.85;

/* -- E1: review canvas ------------------------------------------------------- */

function ReviewCanvasOption({ files }: { files: ExampleFile[] }) {
  const [pinned, setPinned] = useState<string | null>(null);
  const [hovered, setHovered] = useState<string | null>(null);
  const [selected, setSelected] = useState<string | null>(null);
  const [focusNode, setFocusNode] = useState<FocusNode | null>(null);
  const focus = hovered ?? pinned;
  const graph = useMemo(() => buildReviewCanvas(files, focus), [files, focus]);
  const actions = useActions({
    focusFunction: setPinned,
    hoverFunction: setHovered,
    openTrace: setSelected,
    jumpToFile: (fileId) => setFocusNode(jumpRequest(`${fileId}:card`)),
  });

  return (
    <FlowActionsContext.Provider value={actions}>
      <FlowCanvas
        nodes={graph.nodes}
        edges={graph.edges}
        height={canvasHeight(graph, ZOOM)}
        zoom={ZOOM}
        focusNode={focusNode}
      />
      <MarkerLegend />
      <ClickThroughNotice
        selected={selected}
        hint="Hover a function or any chip to light up what connects to it. Click a function to pin it. Click a chip or a footer tab to try the click-through."
      />
    </FlowActionsContext.Provider>
  );
}

/* -- E2: grouped, expand on click -------------------------------------------- */

function GroupedOption({ files }: { files: ExampleFile[] }) {
  const [expanded, setExpanded] = useState<Set<string>>(new Set());
  const graph = useMemo(() => buildGroupedExpandable(files, expanded), [files, expanded]);
  const actions = useActions({
    toggle: (id) =>
      setExpanded((current) => {
        const next = new Set(current);
        if (!next.delete(id)) next.add(id);
        return next;
      }),
  });

  return (
    <FlowActionsContext.Provider value={actions}>
      <FlowCanvas
        nodes={graph.nodes}
        edges={graph.edges}
        height={canvasHeight(graph, ZOOM)}
        zoom={ZOOM}
      />
      <MarkerLegend />
      <ClickThroughNotice
        selected={null}
        hint={`Click a module chip to expand it into its callers or calls. ${expanded.size} expanded.`}
      />
    </FlowActionsContext.Provider>
  );
}

/* -- E3: ledger, refined ------------------------------------------------------ */

function LedgerOption({ files }: { files: ExampleFile[] }) {
  const [sort, setSort] = useState<LedgerSort>("risk");
  const [collapsed, setCollapsed] = useState<Set<string>>(new Set());
  const [selected, setSelected] = useState<string | null>(null);
  const graph = useMemo(
    () => buildLedgerRefined(files, { sort, collapsed, selected }),
    [files, sort, collapsed, selected],
  );
  const actions = useActions({
    openTrace: setSelected,
    toggle: (id) =>
      setCollapsed((current) => {
        const next = new Set(current);
        if (!next.delete(id)) next.add(id);
        return next;
      }),
  });

  return (
    <FlowActionsContext.Provider value={actions}>
      <SummaryStrip files={files}>
        <SegmentedControl
          value={sort}
          onChange={setSort}
          options={[
            { value: "risk", label: "Riskiest first" },
            { value: "path", label: "By path" },
          ]}
        />
      </SummaryStrip>
      <FlowCanvas
        nodes={graph.nodes}
        edges={graph.edges}
        height={canvasHeight(graph, ZOOM)}
        zoom={ZOOM}
      />
      <MarkerLegend />
      <ClickThroughNotice
        selected={selected}
        hint="Click a row header to collapse it. Click any caller or call to try the click-through."
      />
    </FlowActionsContext.Provider>
  );
}

export default function ModuleFlowDepth() {
  return (
    <ExplorationPage
      title="Module flow, depth"
      intro={
        <>
          <p>
            Three ideas from the breadth round, refined and made interactive. Each combines the
            signals that earned their place: function ports (B1), attention markers (C1),
            diff-aware calls (C2) and outputs split by origin (C3).
          </p>
          <p>
            Still hardcoded. All four example files are shown. Clicking a caller or call stands
            in for the later click-through to a call trace.
          </p>
        </>
      }
    >
      <Section
        id="review-canvas"
        title="E1: Review canvas"
        intro="The graph idea, kept readable: one chip per project link, dependencies folded into one list, everything else a click away."
      >
        <Option
          code="E1"
          title="Function ports, markers, diff, folded dependencies"
          note="Inputs and outputs are grouped by the function they connect to, top to bottom in the file's function order, and every column starts at the top of its row. Hover a function or a chip to fade everything unrelated; click a function to pin it. Chips with a link icon jump to the other changed file. The footer tabs are stand-ins for source view and call traces."
        >
          <ReviewCanvasOption files={EXAMPLE_FILES} />
        </Option>
      </Section>

      <Section
        id="grouped"
        title="E2: Grouped, expand on click"
        intro="The same graph with far fewer nodes by default. Detail on demand."
      >
        <Option
          code="E2"
          title="Modules as chips, expanded into functions"
          note="Modules with several callers or calls start as one chip with a count, and thicker edges for more calls. Click one to open it into its functions. Single calls always show directly."
        >
          <GroupedOption files={EXAMPLE_FILES} />
        </Option>
      </Section>

      <Section
        id="ledger"
        title="E3: Ledger"
        intro="The dense, scannable idea: every file a row, no edges to read. It does not need to be a flow diagram."
      >
        <Option
          code="E3"
          title="Ledger rows, riskiest first"
          note="Columns are File, Outputs, Inputs. Hover any line to highlight its changed function with that function's outputs and inputs, and fade the rest. A summary strip on top, rows sorted by risk (stale callers, then removed and renamed functions), collapsible. Calls into dependencies and the standard library are dimmed."
        >
          <LedgerOption files={EXAMPLE_FILES} />
        </Option>
      </Section>
    </ExplorationPage>
  );
}
