import { createContext, useContext } from "react";

/**
 * What nodes can ask the page to do. Nodes stay presentational and never
 * touch the nodes/edges state directly.
 */
export interface FlowActions {
  /** Pin the highlight on one function's callers and calls. Null clears it. */
  focusFunction: (key: string | null) => void;
  /** Highlight a function while the pointer is over it. Null when it leaves. */
  hoverFunction: (key: string | null) => void;
  /** Scroll the canvas to a file's card. */
  jumpToFile: (fileId: string) => void;
  /** Stand-in for the later click-through to a call trace. */
  openTrace: (description: string) => void;
  /** Expand or collapse a group or row. */
  toggle: (id: string) => void;
}

const noop = () => {};

export const FlowActionsContext = createContext<FlowActions>({
  focusFunction: noop,
  hoverFunction: noop,
  jumpToFile: noop,
  openTrace: noop,
  toggle: noop,
});

export function useFlowActions(): FlowActions {
  return useContext(FlowActionsContext);
}
