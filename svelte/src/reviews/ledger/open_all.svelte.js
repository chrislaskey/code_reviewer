// "Open all" and "Close all": one command that every ledger and code block
// follows. `version` counts the commands, so the same command can be sent twice.

import { getContext, setContext } from "svelte";

const CONTEXT_KEY = "review-open-all";

/** Makes the command the page sends. Call once, in the page. */
export function createOpenAllCommand() {
  const command = $state({ version: 0, open: true });
  setContext(CONTEXT_KEY, command);
  return {
    send(open) {
      command.version += 1;
      command.open = open;
    },
  };
}

/**
 * Open or closed state that starts as `initial`, then follows each Open all
 * and Close all command. Setting `open` still changes it afterwards.
 */
export function followOpenAllCommand(initial) {
  const command = getContext(CONTEXT_KEY);
  let open = $state(initial);
  $effect(() => {
    if (command && command.version > 0) open = command.open;
  });
  return {
    get open() {
      return open;
    },
    set open(value) {
      open = value;
    },
  };
}
