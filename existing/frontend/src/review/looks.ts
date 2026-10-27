import type { LedgerStyle } from "../explorations/ledger";
import type { ViewLook } from "../explorations/variants";

/** H5: path subtitles, solid connectors of one colour and width, plain chips. */
export const CHIPS_LOOK: ViewLook = {
  solidEdges: true,
  uniformEdges: true,
  subtleTones: true,
  pathSubtitles: true,
  pathHeader: true,
  coloredFunctions: false,
  functionsLabel: false,
};

/** The file card header shows the file path alone, as in the chips view. */
export const BLOCKS_LOOK: ViewLook = {
  solidEdges: true,
  uniformEdges: true,
  subtleTones: true,
  pathSubtitles: true,
  pathHeader: true,
  coloredFunctions: false,
  functionsLabel: true,
};

/** G1: folded dependencies, two-line callers, counts in the header. */
export const LEDGER_STYLE: LedgerStyle = {
  stackedInputs: true,
  pills: false,
  attention: false,
  coverage: false,
  counts: true,
  order: "by_function",
};
