// What the actions on a session's status bar do (SessionStatusBar), on a
// session's page. Each logs what happened, then calls
// `refresh` so the page shows the session as it is now. Merging or closing a
// session ends it, so on success those call `onFinished` instead, when given.
import {
  sessionsArtifactsOpen,
  sessionsBrowserOpen,
  sessionsDelete,
  sessionsMergesCreate,
  sessionsTerminalWindowsCreate,
} from "../lib/core/index.js";
import { log } from "../lib/code_reviewer/log.svelte.js";

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
        const outcome = await sessionsTerminalWindowsCreate(name);
        log(`${name}: ${SHOW_OUTCOMES[outcome] ?? outcome}`);
      } catch (error) {
        log(`Opening ${name} failed: ${error}`);
      }
      await refresh();
    },

    async openInBrowser(name) {
      try {
        await sessionsBrowserOpen(name);
        log(`${name}: Opened in the browser`);
      } catch (error) {
        log(`Opening ${name} in the browser failed: ${error}`);
      }
    },

    async openArtifactsInBrowser(name) {
      try {
        await sessionsArtifactsOpen(name);
        log(`${name}: Opened its artifacts`);
      } catch (error) {
        log(`Opening the artifacts of ${name} failed: ${error}`);
      }
    },

    merge: (name, intoBranch) => finish(name, (name) => sessionsMergesCreate(name, { into_branch: intoBranch }), "Merging"),

    close: (name) => finish(name, sessionsDelete, "Closing"),

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
