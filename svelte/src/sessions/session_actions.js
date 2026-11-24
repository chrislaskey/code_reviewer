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
import { t } from "ttag";
import { log } from "../lib/code_reviewer/log.svelte.js";

/** What showing a session in the terminal app did, by its outcome. */
export function showOutcomeText(outcome) {
  if (outcome === "focused") return t`Brought its window to the front`;
  if (outcome === "opened") return t`Opened a new window`;
  return outcome;
}

/**
 * @param {{ refresh: () => Promise<unknown>, onFinished?: (name: string) => unknown }} options
 */
export function createSessionActions({ refresh, onFinished }) {
  async function finish(name, action, failureText) {
    try {
      log(await action(name));
    } catch (error) {
      log(failureText(error));
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
        const outcomeText = showOutcomeText(outcome);
        log(t`${name}: ${outcomeText}`);
      } catch (error) {
        log(t`Opening ${name} failed: ${error}`);
      }
      await refresh();
    },

    async openInBrowser(name) {
      try {
        await sessionsBrowserOpen(name);
        log(t`${name}: Opened in the browser`);
      } catch (error) {
        log(t`Opening ${name} in the browser failed: ${error}`);
      }
    },

    async openArtifactsInBrowser(name) {
      try {
        await sessionsArtifactsOpen(name);
        log(t`${name}: Opened its artifacts`);
      } catch (error) {
        log(t`Opening the artifacts of ${name} failed: ${error}`);
      }
    },

    merge: (name, intoBranch) =>
      finish(
        name,
        (name) => sessionsMergesCreate(name, { into_branch: intoBranch }),
        (error) => t`Merging ${name} failed: ${error}`,
      ),

    close: (name) => finish(name, sessionsDelete, (error) => t`Closing ${name} failed: ${error}`),

    // The clipboard API can be unavailable in the webview, so fall back to
    // selecting the text for the user to copy. Returns whether it copied.
    async copyText(text, element) {
      try {
        await navigator.clipboard.writeText(text);
        log(t`Copied: ${text}`);
        return true;
      } catch {
        window.getSelection().selectAllChildren(element);
        log(t`Could not copy, the command is selected instead`);
        return false;
      }
    },
  };
}
