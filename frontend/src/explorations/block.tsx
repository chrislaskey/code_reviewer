import { Handle, Position, type Node, type NodeProps } from "@xyflow/react";
import { useFlowActions } from "./actions";
import { SIZES } from "./layout";
import { MarkerIcon, type Marker } from "./nodes";
import type { ChipTone } from "./style";

/*
 * Many callers or calls in one box with a single connection point. Hovering a
 * row lights up the rest of its changed function inside the box.
 */

export type BlockRow =
  | {
      kind: "item";
      title: string;
      detail?: string;
      tone?: ChipTone;
      markers?: Marker[];
      /** Quieter text, for calls that rarely need review. */
      dim?: boolean;
      /** `fileId:name/arity` of the changed function this row belongs to. */
      focusKey: string;
      jumpToFileId?: string;
    }
  | { kind: "heading"; label: string; focusKey: string }
  | { kind: "note"; label: string };

export type BlockData = {
  side: "input" | "output";
  title: string;
  rows: BlockRow[];
  /** The function being hovered or pinned, as `fileId:name/arity`. */
  focus: string | null;
};

const ROW_HEIGHTS = { item: 26, heading: 26, note: 22 } as const;

export function blockHeight(rows: BlockRow[]): number {
  const content = rows.reduce((sum, row) => sum + ROW_HEIGHTS[row.kind], 0);
  return SIZES.listHeader + content + SIZES.listPadding;
}

export function BlockNode({ data }: NodeProps<Node<BlockData, "block">>) {
  const actions = useFlowActions();
  const { focus } = data;

  const faded = (focusKey: string) => focus !== null && focus !== focusKey;

  return (
    <div
      style={{ width: SIZES.blockWidth }}
      className="rounded-lg border border-base-300 bg-base-100 shadow-sm"
    >
      <div
        style={{ height: SIZES.listHeader }}
        className="flex items-center px-2.5 text-[10px] font-semibold uppercase tracking-wide text-base-content/50"
      >
        {data.title}
      </div>
      <ul style={{ paddingBottom: SIZES.listPadding }}>
        {data.rows.map((row, index) => {
          if (row.kind === "note") {
            return (
              <li
                key={`note-${index}`}
                style={{ height: ROW_HEIGHTS.note }}
                className="flex items-center px-2.5 font-mono text-[10px] text-base-content/40"
              >
                {row.label}
              </li>
            );
          }
          const common = {
            onMouseEnter: () => actions.hoverFunction(row.focusKey),
            onMouseLeave: () => actions.hoverFunction(null),
          };
          if (row.kind === "heading") {
            return (
              <li
                key={`heading-${index}`}
                {...common}
                style={{ height: ROW_HEIGHTS.heading }}
                className={[
                  "nodrag nopan flex items-end px-2.5 pb-0.5 font-mono text-[10px] font-semibold text-base-content/50 transition-opacity duration-150",
                  faded(row.focusKey) ? "opacity-25" : "",
                ].join(" ")}
              >
                {row.label}
              </li>
            );
          }
          const onClick = () =>
            row.jumpToFileId ? actions.jumpToFile(row.jumpToFileId) : actions.openTrace(row.title);
          return (
            <li
              key={`item-${index}-${row.title}`}
              {...common}
              onClick={onClick}
              style={{ height: ROW_HEIGHTS.item }}
              title={row.title}
              className={[
                "nodrag nopan flex cursor-pointer items-center gap-1.5 px-2.5 font-mono text-[11px] transition-[opacity,background-color] duration-150",
                focus === row.focusKey ? "bg-base-content/10" : "hover:bg-base-content/5",
                faded(row.focusKey) ? "opacity-25" : "",
                row.dim ? "text-base-content/60" : "",
                row.tone === "stale" ? "text-rose-700 dark:text-rose-300" : "",
                row.tone === "removed" ? "line-through decoration-rose-500/60" : "",
              ].join(" ")}
            >
              <span className="min-w-0 flex-1 truncate">{row.title}</span>
              {row.detail && (
                <span className="shrink-0 text-[10px] text-base-content/50">{row.detail}</span>
              )}
              {row.markers?.map((marker) => <MarkerIcon key={marker} marker={marker} />)}
            </li>
          );
        })}
      </ul>
      <Handle
        type={data.side === "input" ? "source" : "target"}
        position={data.side === "input" ? Position.Right : Position.Left}
      />
    </div>
  );
}
