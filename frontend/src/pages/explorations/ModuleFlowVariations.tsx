import { useMemo, useState } from "react";
import { FlowActionsContext } from "../../explorations/actions";
import { FlowCanvas, type FocusNode } from "../../explorations/canvas";
import {
  EXAMPLE_FILES,
  OUTSIDE_MODE_OPTIONS,
  type ExampleFile,
  type OutsideMode,
} from "../../explorations/data";
import {
  ClickThroughNotice,
  MarkerLegend,
  SegmentedControl,
  SummaryStrip,
  Toolbar,
  jumpRequest,
  toggled,
  useActions,
} from "../../explorations/controls";
import type { LedgerSort } from "../../explorations/depth";
import type { Graph } from "../../explorations/layout";
import { canvasHeight } from "../../explorations/layout";
import type { LedgerStyle } from "../../explorations/ledger";
import { ExplorationPage, Option, Section } from "../../explorations/scaffold";
import {
  buildBlocksOnePoint,
  buildChipsOnePoint,
  buildLedgerVariant,
  type ViewLook,
} from "../../explorations/variants";

const ZOOM = 0.85;
/** The block views are wider, so they open a little smaller. */
const BLOCKS_ZOOM = 0.8;

function OutsideControl({
  value,
  onChange,
}: {
  value: OutsideMode;
  onChange: (value: OutsideMode) => void;
}) {
  return (
    <SegmentedControl
      label="Dependencies and standard library"
      value={value}
      onChange={onChange}
      options={OUTSIDE_MODE_OPTIONS}
    />
  );
}

/* -- Ledger variations (E3) --------------------------------------------------- */

const LEDGER_STYLES = {
  folded: {
    stackedInputs: true,
    pills: false,
    attention: false,
    coverage: false,
    counts: true,
    order: "by_function",
  },
  alphabetical: {
    stackedInputs: true,
    pills: false,
    attention: false,
    coverage: false,
    counts: true,
    order: "alphabetical",
  },
  attention: {
    stackedInputs: false,
    pills: false,
    attention: true,
    coverage: false,
    counts: true,
    order: "by_function",
  },
  pills: {
    stackedInputs: false,
    pills: true,
    attention: false,
    coverage: false,
    counts: true,
    order: "by_function",
  },
  combined: {
    stackedInputs: true,
    pills: false,
    attention: true,
    coverage: true,
    counts: false,
    order: "by_function",
  },
} satisfies Record<string, LedgerStyle>;

function LedgerVariant({
  files,
  style,
  withSummary = false,
}: {
  files: ExampleFile[];
  style: LedgerStyle;
  withSummary?: boolean;
}) {
  const [outside, setOutside] = useState<OutsideMode>("dimmed");
  const [sort, setSort] = useState<LedgerSort>("risk");
  const [collapsed, setCollapsed] = useState<Set<string>>(new Set());
  const [selected, setSelected] = useState<string | null>(null);
  const graph = useMemo(
    () => buildLedgerVariant(files, { style, outside, sort, collapsed, selected }),
    [files, style, outside, sort, collapsed, selected],
  );
  const actions = useActions({
    openTrace: setSelected,
    toggle: (id) => setCollapsed((current) => toggled(current, id)),
  });

  const controls = (
    <>
      <OutsideControl value={outside} onChange={setOutside} />
      {withSummary && (
        <SegmentedControl
          value={sort}
          onChange={setSort}
          options={[
            { value: "risk", label: "Riskiest first" },
            { value: "path", label: "By path" },
          ]}
        />
      )}
    </>
  );

  return (
    <FlowActionsContext.Provider value={actions}>
      {withSummary ? (
        <SummaryStrip files={files}>
          <div className="flex flex-wrap items-center gap-x-6 gap-y-2">{controls}</div>
        </SummaryStrip>
      ) : (
        <Toolbar>{controls}</Toolbar>
      )}
      <FlowCanvas
        nodes={graph.nodes}
        edges={graph.edges}
        height={canvasHeight(graph, ZOOM)}
        zoom={ZOOM}
      />
      <MarkerLegend />
      <ClickThroughNotice
        selected={selected}
        hint="Hover a function, call, caller or finding to light up everything on its function. Click a header to collapse a file. Click a line to try the click-through."
      />
    </FlowActionsContext.Provider>
  );
}

/* -- Node variations (A1, A2) ------------------------------------------------- */

type BuildNodeView = (files: ExampleFile[], focusKey: string | null, outside: OutsideMode) => Graph;

const CHIPS_COVERAGE: BuildNodeView = (files, focusKey, outside) =>
  buildChipsOnePoint(files, { focusKey, outside, stats: "coverage", compact: false });

const CHIPS_COMPACT: BuildNodeView = (files, focusKey, outside) =>
  buildChipsOnePoint(files, { focusKey, outside, stats: "none", compact: true });

/** H1 with the extras cut: paths only, solid lines, quiet tones, tiny coverage. */
const SIMPLE_LOOK: ViewLook = {
  solidEdges: true,
  subtleTones: true,
  pathSubtitles: true,
  pathHeader: true,
  coloredFunctions: false,
  functionsLabel: false,
};

const CHIPS_SIMPLE: BuildNodeView = (files, focusKey, outside) =>
  buildChipsOnePoint(files, {
    focusKey,
    outside,
    stats: "none",
    compact: false,
    look: SIMPLE_LOOK,
  });

const CHIPS_SIMPLE_COLORED: BuildNodeView = (files, focusKey, outside) =>
  buildChipsOnePoint(files, {
    focusKey,
    outside,
    stats: "none",
    compact: false,
    look: { ...SIMPLE_LOOK, coloredFunctions: true },
  });

const BLOCKS_COVERAGE: BuildNodeView = (files, focusKey, outside) =>
  buildBlocksOnePoint(files, { focusKey, outside, stats: "coverage", groupedByFunction: false });

const BLOCKS_BY_FUNCTION: BuildNodeView = (files, focusKey, outside) =>
  buildBlocksOnePoint(files, { focusKey, outside, stats: "none", groupedByFunction: true });

function NodeVariant({
  files,
  build,
  zoom = ZOOM,
}: {
  files: ExampleFile[];
  build: BuildNodeView;
  zoom?: number;
}) {
  const [outside, setOutside] = useState<OutsideMode>("dimmed");
  const [pinned, setPinned] = useState<string | null>(null);
  const [hovered, setHovered] = useState<string | null>(null);
  const [selected, setSelected] = useState<string | null>(null);
  const [focusNode, setFocusNode] = useState<FocusNode | null>(null);
  const focus = hovered ?? pinned;
  const graph = useMemo(() => build(files, focus, outside), [build, files, focus, outside]);
  const actions = useActions({
    focusFunction: setPinned,
    hoverFunction: setHovered,
    openTrace: setSelected,
    jumpToFile: (fileId) => setFocusNode(jumpRequest(`${fileId}:card`)),
  });

  return (
    <FlowActionsContext.Provider value={actions}>
      <Toolbar>
        <OutsideControl value={outside} onChange={setOutside} />
      </Toolbar>
      <FlowCanvas
        nodes={graph.nodes}
        edges={graph.edges}
        height={canvasHeight(graph, zoom)}
        zoom={zoom}
        focusNode={focusNode}
      />
      <MarkerLegend />
      <ClickThroughNotice
        selected={selected}
        hint="Hover a function, caller or call to light up its connections. Click a function to pin it. Click a chip to try the click-through, or a link icon to jump to the other file."
      />
    </FlowActionsContext.Provider>
  );
}

export default function ModuleFlowVariations() {
  return (
    <ExplorationPage
      title="Module flow, variations"
      intro={
        <>
          <p>
            A third round, still hardcoded. The ledger (E3) picks up pieces from the ledger
            variants page, and the A1 and A2 node views are redone with one connection point per
            side and hover to show which connections belong to a function.
          </p>
          <p>
            Every view here is a ReactFlow canvas, so a later switcher can move between them.
            Dependencies and the standard library are dimmed by default; each view can fold them
            into one line or hide them. Test coverage is made-up example data.
          </p>
        </>
      }
    >
      <Section
        id="ledger"
        title="E3 ledger, iterated"
        intro="Still one row per file with File / Outputs / Inputs as independent columns, not a row per function."
      >
        <Option
          code="G1"
          title="Folded dependencies, two-line callers"
          note="E3 with the dependency control and callers on two lines (name, then file and line) from F1. Folded turns the dependency calls into one count under each file's outputs."
        >
          <LedgerVariant files={EXAMPLE_FILES} style={LEDGER_STYLES.folded} />
        </Option>
        <Option
          code="G1b"
          title="Alphabetical instead of grouped"
          note="G1 with outputs and inputs sorted by name, not grouped under their function. Dimmed dependencies sit among the project calls, and the function each call belongs to is the grey text on the right. Try it in Dimmed, then Folded, and compare with G1."
        >
          <LedgerVariant files={EXAMPLE_FILES} style={LEDGER_STYLES.alphabetical} />
        </Option>
        <Option
          code="G2"
          title="Attention strip"
          note="F3's findings move into each file: stale callers and functions with new project calls as chips under the header. Hover a chip to light up that function's lines. Stays visible when the file is collapsed, so a collapsed ledger still shows where to look."
        >
          <LedgerVariant files={EXAMPLE_FILES} style={LEDGER_STYLES.attention} />
        </Option>
        <Option
          code="G3"
          title="Pills in columns"
          note="F2's pills inside the E3 columns. Much shorter rows, and the pill colours carry stale, test and new-call. Callers lose their file and line."
        >
          <LedgerVariant files={EXAMPLE_FILES} style={LEDGER_STYLES.pills} />
        </Option>
        <Option
          code="G4"
          title="Combined, with coverage"
          note="Two-line callers, the attention strip, folded dependencies, a summary strip with sorting, and test coverage in each header instead of the counts. The candidate if G1 to G3 all earn their place."
        >
          <LedgerVariant files={EXAMPLE_FILES} style={LEDGER_STYLES.combined} withSummary />
        </Option>
      </Section>

      <Section
        id="chips"
        title="A2 chips, iterated"
        intro="One chip per caller and call, one connection point on each side of the card. Hover shows which chips and lines belong to a function."
      >
        <Option
          code="H1"
          title="Chips, one point, coverage in the card"
          note="The card's counts row is replaced by test coverage. Everything unrelated to the hovered function fades and its lines thicken. Dependencies show as muted chips with dashed lines."
        >
          <NodeVariant files={EXAMPLE_FILES} build={CHIPS_COVERAGE} />
        </Option>
        <Option
          code="H5"
          title="H1, simplified"
          note="Coverage shrinks to a dot and a percentage beside the module name. Under the module and on every chip the subtitle is just the file path, clipped at the start so the file name stays visible. All lines are solid, with dependencies thinner and lighter. Changed calls are plain boxes with just their icon, and connectors are grey except for stale callers. The Changed functions label is gone."
        >
          <NodeVariant files={EXAMPLE_FILES} build={CHIPS_SIMPLE} />
        </Option>
        <Option
          code="H6"
          title="H5, with coloured function names"
          note="H5 with the function names in the card coloured by what changed: amber modified, green added, red removed, violet renamed. Deeper shades in light mode and softer ones in dark mode so the text stays readable."
        >
          <NodeVariant files={EXAMPLE_FILES} build={CHIPS_SIMPLE_COLORED} />
        </Option>
        <Option
          code="H2"
          title="Compact chips in function bands, no stats row"
          note="One-line chips, with extra space between functions so each function's callers and calls form a band. The card has no stats row at all; file and line are in the tooltip. Much shorter."
        >
          <NodeVariant files={EXAMPLE_FILES} build={CHIPS_COMPACT} />
        </Option>
      </Section>

      <Section
        id="blocks"
        title="A1 blocks, iterated"
        intro="One box of callers and one of calls per file, one connection point each. Hover a row to light up its function inside the box."
      >
        <Option
          code="H3"
          title="Blocks, one point, coverage in the card"
          note="A1 with test coverage in place of the counts. Hover a row, or a function in the card, to fade every row that belongs to another function. Dependencies are muted rows."
        >
          <NodeVariant files={EXAMPLE_FILES} build={BLOCKS_COVERAGE} zoom={BLOCKS_ZOOM} />
        </Option>
        <Option
          code="H4"
          title="Blocks grouped under function headings, no stats row"
          note="Rows sit under a heading per changed function, so the grouping is visible without hovering. No stats row. Closest to the ledger while still a node view."
        >
          <NodeVariant files={EXAMPLE_FILES} build={BLOCKS_BY_FUNCTION} zoom={BLOCKS_ZOOM} />
        </Option>
      </Section>
    </ExplorationPage>
  );
}
