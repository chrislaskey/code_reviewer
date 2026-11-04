// What the actions on a session's status bar do (SessionStatusBar), shared by
// the sessions page and a session's page. Each logs what happened, then calls
// `refresh` so the page shows the session as it is now. Merging or closing a
// session ends it, so on success those call `onFinished` instead, when given.
import {
  closeSession,
  mergeSession,
  openSessionInBrowser,
  showSessionInTerminalApp,
} from "../lib/backend.js";
import { log } from "../lib/log.svelte.js";

export const SHOW_OUTCOMES = {
  focused: "Brought its window to the front",
  opened: "Opened a new window",
};

/**
 * @param {{ refresh: () => Promise<unknown>, onFinished?: (name: string) => unknown }} options
 */
export function createSessionActions({ refresh, onFinished }) {
  async function finish(name, action, failure) {
    try {
      log(await action(name));
    } catch (error) {
      log(`${failure} ${name} failed: ${error}`);
      await refresh();
      return;
    }
    if (onFinished) await onFinished(name);
    else await refresh();
  }

  return {
    async showInTerminalApp(name) {
      try {
        const outcome = await showSessionInTerminalApp(name);
        log(`${name}: ${SHOW_OUTCOMES[outcome] ?? outcome}`);
      } catch (error) {
        log(`Opening ${name} failed: ${error}`);
      }
      await refresh();
    },

    async openInBrowser(name) {
      try {
        log(`${name}: Opened ${await openSessionInBrowser(name)}`);
      } catch (error) {
        log(`Opening ${name} in the browser failed: ${error}`);
      }
    },

    merge: (name) => finish(name, mergeSession, "Merging"),

    close: (name) => finish(name, closeSession, "Closing"),

    // The clipboard API can be unavailable in the webview, so fall back to
    // selecting the text for the user to copy. Returns whether it copied.
    async copyText(text, element) {
      try {
        await navigator.clipboard.writeText(text);
        log(`Copied: ${text}`);
        return true;
      } catch {
        window.getSelection().selectAllChildren(element);
        log("Could not copy, the command is selected instead");
        return false;
      }
    },
  };
}
