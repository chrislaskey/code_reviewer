// How the files index shows its files: "list", a full-width row each, or
// "grid", small squares many to a screen. The choice is kept in
// localStorage under `app:files-view`; absent, it is "list".

const FILES_VIEW_KEY = "app:files-view";

/** @typedef {"list" | "grid"} FilesView */

/** @returns {FilesView} */
export function filesView() {
  return localStorage.getItem(FILES_VIEW_KEY) === "grid" ? "grid" : "list";
}

/** @param {FilesView} view */
export function setFilesView(view) {
  localStorage.setItem(FILES_VIEW_KEY, view);
}
