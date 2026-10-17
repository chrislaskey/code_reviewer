import { BlockNode } from "./block";
import { LedgerNode } from "./ledger";
import { ChipNode, FileCardNode, GroupNode, ListNode, RowNode } from "./nodes";

/** One stable map for every explorations canvas. */
export const NODE_TYPES = {
  chip: ChipNode,
  list: ListNode,
  module_group: GroupNode,
  file_card: FileCardNode,
  row: RowNode,
  ledger: LedgerNode,
  block: BlockNode,
};
