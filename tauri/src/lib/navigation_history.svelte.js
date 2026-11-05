// Where the window is in its navigation history, read from the webview's
// Navigation API, so it follows every way the address changes (the router's
// links, `history.back()`). Like a browser's, the history is a line: going
// to a new page after going back drops the pages that were ahead.

export const navigationHistory = $state({ canGoBack: false, canGoForward: false });

function readNavigationHistory() {
  navigationHistory.canGoBack = window.navigation?.canGoBack ?? false;
  navigationHistory.canGoForward = window.navigation?.canGoForward ?? false;
}

/** Keeps `navigationHistory` current; returns the function that stops it. */
export function trackNavigationHistory() {
  readNavigationHistory();
  window.navigation?.addEventListener("currententrychange", readNavigationHistory);
  return () => window.navigation?.removeEventListener("currententrychange", readNavigationHistory);
}

/** Shows the previous page, if there is one. */
export function goBackInHistory() {
  if (navigationHistory.canGoBack) window.history.back();
}

/** Shows the page the user went back from, if there is one. */
export function goForwardInHistory() {
  if (navigationHistory.canGoForward) window.history.forward();
}
