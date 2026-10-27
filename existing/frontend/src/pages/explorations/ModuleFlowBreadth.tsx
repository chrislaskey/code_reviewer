import { FlowCanvas } from "../../explorations/canvas";
import {
  buildChips,
  buildFileMap,
  buildGrouped,
  buildLedger,
  buildOriginLanes,
  buildPorts,
  buildThreeBlocks,
} from "../../explorations/breadth";
import { EXAMPLE_FILES, type ExampleFile } from "../../explorations/data";
import type { Graph } from "../../explorations/layout";
import { ExplorationPage, Legend, Option, Section } from "../../explorations/scaffold";

function fileById(id: string): ExampleFile {
  const file = EXAMPLE_FILES.find((candidate) => candidate.id === id);
  if (!file) throw new Error(`Missing example file ${id}`);
  return file;
}

/** Two files keep each canvas readable; the ledger and file map show all four. */
const SAMPLE = [fileById("orders"), fileById("inventory")];

interface Idea {
  code: string;
  title: string;
  note: string;
  graph: Graph;
  height: number;
  legend?: { swatch: string; label: string }[];
}

const STATUS_LEGEND = [
  { swatch: "bg-emerald-500", label: "new call or added function" },
  { swatch: "bg-amber-400", label: "modified" },
  { swatch: "bg-rose-500", label: "removed, or caller of something removed" },
  { swatch: "bg-violet-500", label: "renamed" },
];

const IDEAS: { id: string; title: string; intro: string; ideas: Idea[] }[] = [
  {
    id: "shape",
    title: "Shape: how the three columns are drawn",
    intro:
      "The same data with the least and the most structure. How much of the graph is nodes, and how much is list text?",
    ideas: [
      {
        code: "A1",
        title: "Three blocks per row",
        note: "One list for inputs, the file card, one list for outputs. Calmest, fewest edges, but every edge says nothing: you cannot tell which caller reaches which function.",
        graph: buildThreeBlocks(SAMPLE),
        height: 640,
      },
      {
        code: "A2",
        title: "One chip per link",
        note: "Every caller and every call is its own node with its own edge. Honest about the data, but a file with many calls fans out into spaghetti.",
        graph: buildChips(SAMPLE, false),
        height: 720,
      },
      {
        code: "A3",
        title: "Ledger rows",
        note: "One wide row per file with File, Outputs and Inputs columns, and no edges. Hover any line to highlight its function with its outputs and inputs, and fade the rest. Maximum density and perfect alignment, but nothing to follow by eye.",
        graph: buildLedger(EXAMPLE_FILES),
        height: 720,
      },
    ],
  },
  {
    id: "attribution",
    title: "Attribution: which function does what",
    intro:
      "A file changes several functions. Which caller reaches which function, and which call comes from which?",
    ideas: [
      {
        code: "B1",
        title: "Function ports",
        note: "The card lists its changed functions, and each has its own handle. Edges land on the function they call. The first idea that answers 'which function does this affect?'.",
        graph: buildPorts(SAMPLE, false),
        height: 720,
      },
      {
        code: "B2",
        title: "Grouped by module",
        note: "Callers and calls collapse into one chip per module with a count, and the edge gets thicker with more calls. Far fewer nodes, and heavy coupling shows at a glance.",
        graph: buildGrouped(SAMPLE),
        height: 560,
      },
    ],
  },
  {
    id: "review-signals",
    title: "Review signals: what deserves attention",
    intro:
      "Colour means status and nothing else. These add what a reviewer needs to know about each link.",
    ideas: [
      {
        code: "C1",
        title: "Attention markers on callers",
        note: "Stale callers (still calling something removed or renamed) go red, tests go dashed and grey, callers that are also changed get a link icon. Answers 'what could this break?'.",
        graph: buildChips(SAMPLE, true),
        height: 720,
        legend: [
          { swatch: "bg-rose-500/60", label: "stale caller" },
          { swatch: "border border-dashed border-base-content/40", label: "test" },
          { swatch: "bg-amber-500", label: "also changed" },
        ],
      },
      {
        code: "C2",
        title: "Diff-aware calls",
        note: "Calls added by this change are green, calls it removed are struck through with a dashed edge, the rest stay quiet. Answers 'what does this change newly depend on?'. Needs a base-vs-head diff of calls the backend does not return yet.",
        graph: buildPorts(SAMPLE, true),
        height: 720,
        legend: STATUS_LEGEND,
      },
      {
        code: "C3",
        title: "Outputs split by origin",
        note: "Calls into the project stay as chips. Dependencies and the standard library collapse into two small lists behind a thin dashed edge. Most calls are noise, and this hides it without losing it.",
        graph: buildOriginLanes(SAMPLE),
        height: 720,
      },
    ],
  },
  {
    id: "between-files",
    title: "Between files",
    intro: "Changed files call each other. Rows show each file alone and repeat those links.",
    ideas: [
      {
        code: "D1",
        title: "Changed files connected",
        note: "Only the changed files, as cards, joined by the calls between them, ordered by who calls whom. Shows the shape of the whole change. Leaves out the callers and calls outside it.",
        graph: buildFileMap(EXAMPLE_FILES),
        height: 520,
      },
    ],
  },
];

export default function ModuleFlowBreadth() {
  return (
    <ExplorationPage
      title="Module flow, breadth"
      intro={
        <>
          <p>
            Inputs (callers from other modules) → file → outputs (calls into other modules), one
            level deep, drawn many ways with the same hardcoded example.
          </p>
          <p>
            Each canvas pans and zooms. Hold Ctrl or Cmd and scroll to zoom without moving the
            page.
          </p>
        </>
      }
    >
      {IDEAS.map((section) => (
        <Section key={section.id} id={section.id} title={section.title} intro={section.intro}>
          {section.ideas.map((idea) => (
            <Option key={idea.code} code={idea.code} title={idea.title} note={idea.note}>
              <FlowCanvas nodes={idea.graph.nodes} edges={idea.graph.edges} height={idea.height} />
              {idea.legend && (
                <Legend
                  items={idea.legend.map((item) => ({
                    label: item.label,
                    swatch: <span className={`size-2.5 rounded-full ${item.swatch}`} />,
                  }))}
                />
              )}
            </Option>
          ))}
        </Section>
      ))}
    </ExplorationPage>
  );
}
