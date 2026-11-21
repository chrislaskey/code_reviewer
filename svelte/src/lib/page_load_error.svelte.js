// What went wrong getting the page ready, shown above it by the layout
// (RemountPageOnNavigation.svelte) until the next navigation: the router
// could not tell who is signed in, or the page's code did not load.
import { errorMessageOf } from "./error_message.js";

export const pageLoadError = $state({ message: "" });

/** @param {unknown} problem */
export function pageLoadErrorShow(problem) {
  pageLoadError.message = errorMessageOf(problem);
}

export function pageLoadErrorClear() {
  pageLoadError.message = "";
}
