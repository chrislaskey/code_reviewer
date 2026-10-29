// The router: the page at each path, swapped in place as the address
// changes, so switching pages never shows a blank window. The paths are
// routes.js's, the one module that spells them out. A page is loaded the
// first time it is visited, so each page's code stays a chunk of its own,
// as it was when each page had an HTML file.
import { createRouter } from "sv-router";
import RemountPageOnNavigation from "./components/RemountPageOnNavigation.svelte";
import {
  HOME_PATH,
  REVIEW_NEW_PATH,
  REVIEW_SHOW_PATH,
  SESSION_NEW_PATH,
  SESSIONS_INDEX_PATH,
  TRACE_NEW_PATH,
  TRACE_SHOW_PATH,
} from "./routes.js";

const { navigate } = createRouter({
  layout: RemountPageOnNavigation,
  [HOME_PATH]: () => import("../HomePage.svelte"),
  [SESSIONS_INDEX_PATH]: () => import("../sessions/SessionsIndexPage.svelte"),
  [SESSION_NEW_PATH]: () => import("../sessions/SessionNewPage.svelte"),
  [REVIEW_NEW_PATH]: () => import("../reviews/ReviewNewPage.svelte"),
  [REVIEW_SHOW_PATH]: () => import("../reviews/ReviewShowPage.svelte"),
  [TRACE_NEW_PATH]: () => import("../traces/TraceNewPage.svelte"),
  [TRACE_SHOW_PATH]: () => import("../traces/TraceShowPage.svelte"),
});

// The router types its paths; ours come from routes.js, so any path goes.
const navigateToPath = /** @type {(pathname: string, options: { search: string }) => Promise<unknown>} */ (
  /** @type {unknown} */ (navigate)
);

/**
 * Shows the page at a URL from routes.js, such as `/reviews/show?repo_path=…`,
 * in place of the current one, and adds it to the history as a link would.
 * @param {string} url
 */
export function navigateToUrl(url) {
  const { pathname, search } = new URL(url, window.location.origin);
  return navigateToPath(pathname, { search });
}
