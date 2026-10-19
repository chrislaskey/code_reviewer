import { useState } from "react";
import { SegmentedControl, Toolbar } from "../../explorations/controls";
import { buildGroupRow, type GroupRow } from "../../explorations/group_rows";
import {
  FunctionsView,
  GroupRows,
  RENDERER_OPTIONS,
  type GroupRowHandlers,
  type Renderer,
} from "../../explorations/group_views";
import {
  ALL_CHANGES_ID,
  EXAMPLE_GROUPS,
  GROUPED_FILES,
  filesInGroup,
  summarizeFiles,
} from "../../explorations/groups_data";
import { ExplorationPage, Option, Section } from "../../explorations/scaffold";
import { GroupedReview } from "../../review/GroupedReview";

const FILES = GROUPED_FILES;
const GROUPS = EXAMPLE_GROUPS;

const ROWS: GroupRow[] = GROUPS.map((group) => buildGroupRow(FILES, GROUPS, group.id));

const ALL_ROW: GroupRow = {
  group: { id: ALL_CHANGES_ID, label: "All changes by group" },
  stats: summarizeFiles(FILES),
  status: "modified",
  content_label: "Groups",
  lines: GROUPS.map((group) => ({
    key: group.id,
    kind: "group",
    id: group.id,
    label: group.label,
    stats: summarizeFiles(filesInGroup(FILES, group.id)),
  })),
  outputs: [],
  inputs: [],
  folded_output_count: 0,
};

/** Static rows ignore clicks. */
const NO_HANDLERS: GroupRowHandlers = { onOpenLine: () => {}, onOpenLink: () => {} };

function OneLevelUp({ renderer }: { renderer: Renderer }) {
  const orders = FILES.find((file) => file.id === "orders");
  return (
    <div>
      <p className="border-b border-base-300 bg-base-100 px-4 py-1.5 text-[11px] font-medium text-base-content/60">
        Functions view: one file
      </p>
      {orders && <FunctionsView files={[orders]} renderer={renderer} />}
      <p className="border-y border-base-300 bg-base-100 px-4 py-1.5 text-[11px] font-medium text-base-content/60">
        Groups view: the group that holds it, Business logic
      </p>
      <GroupRows
        rows={ROWS.filter((row) => row.group.id === "business")}
        renderer={renderer}
        handlers={NO_HANDLERS}
      />
    </div>
  );
}

export default function GroupsFlow() {
  const [renderer, setRenderer] = useState<Renderer>("ledger_one_code_block");
  return (
    <ExplorationPage
      title="Groups"
      intro={
        <>
          <p>
            Groups are one flat list. The Groups view is the file view one level up: the same
            ledger or node row, with a group in the header and its files as the content. The
            outputs and inputs are the files outside the group. Clicking a file opens the
            Functions view, which is the view the app has for a single file, for every file in
            that group.
          </p>
          <p>
            A file can be in many groups. When any group exists, files in none of them are in an
            implicit default group. Everything is hardcoded; the review page at
            /review/html uses the same component with real data.
          </p>
        </>
      }
    >
      <Section id="groups-view" title="The Groups view">
        <Option
          code="V1"
          title="One level up"
          note="Hover a file or link to highlight what it connects to, as in the file ledger. Click a linked file that is changed in this review to jump to its group. Dependencies and the standard library are folded, because at file level they are noise."
        >
          <Toolbar>
            <SegmentedControl label="View" value={renderer} onChange={setRenderer} options={RENDERER_OPTIONS} />
          </Toolbar>
          <OneLevelUp renderer={renderer} />
        </Option>
      </Section>

      <Section id="flow" title="Flow">
        <Option
          code="F1"
          title="Tree that starts with groups"
          clip={false}
          note="One scrolling page. It opens with All changes, which lists every group, then a row for each group. The tree lists All changes and the groups. Click a file line to go to the Functions view of that group. The tree then adds that group's files under it, and picking another group switches the Functions view to it. The link at the top of the tree returns to the groups."
        >
          <GroupedReview groups={GROUPS} files={FILES} allRow={ALL_ROW} rows={ROWS} />
        </Option>
      </Section>
    </ExplorationPage>
  );
}
