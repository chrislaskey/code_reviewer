// Every page's address, so pages link to each other through here and never
// spell out a path or a query parameter themselves. A page takes what it
// shows from its query, flat and named, the way a web server would read it:
// `/reviews/show.html?repo_path=…&base_ref=main&head_ref=cr-fix&compare_from_merge_base=true`,
// with a list as a repeated key. So a page with its input is a plain link,
// from another page, from a bookmark, or later from a web server.

export function homeUrl() {
  return "/index.html";
}

export function sessionsIndexUrl() {
  return "/sessions/index.html";
}

export function sessionNewUrl() {
  return "/sessions/new.html";
}

/** The new review page, with its form filled in from a request when given one. */
export function reviewNewUrl(request = null) {
  return withQuery("/reviews/new.html", request ? reviewRequestToParams(request) : null);
}

/** The show page, running a request (the core crate's `analyze_file_changes` request). */
export function reviewShowUrl(request) {
  return withQuery("/reviews/show.html", reviewRequestToParams(request));
}

function withQuery(path, params) {
  const query = params?.toString() ?? "";
  return query === "" ? path : `${path}?${query}`;
}

/* -- A review request as query parameters --------------------------------------- */

/** @param {object} request the core crate's AnalyzeFileChangesRequest, with snake_case fields. */
export function reviewRequestToParams(request) {
  const params = new URLSearchParams();
  params.set("repo_path", request.repo_path);
  if (request.base_ref) params.set("base_ref", request.base_ref);
  if (request.head_ref) params.set("head_ref", request.head_ref);
  if (request.compare_from_merge_base) params.set("compare_from_merge_base", "true");
  for (const rule of request.module_rules?.include ?? []) params.append("module_include", rule);
  for (const rule of request.module_rules?.ignore ?? []) params.append("module_ignore", rule);
  if (request.group_flags?.group_config_path) params.set("group_config_path", request.group_flags.group_config_path);
  for (const entry of request.group_flags?.include ?? []) params.append("group_include", entry);
  for (const entry of request.group_flags?.exclude ?? []) params.append("group_exclude", entry);
  return params;
}

/** The request in a page's query (`location.search`), or null when it names no repository. */
export function reviewRequestFromSearch(search) {
  const params = new URLSearchParams(search);
  const repo_path = params.get("repo_path");
  if (!repo_path) return null;
  const request = {
    repo_path,
    module_rules: { include: params.getAll("module_include"), ignore: params.getAll("module_ignore") },
    group_flags: {
      group_config_path: params.get("group_config_path") || null,
      include: params.getAll("group_include"),
      exclude: params.getAll("group_exclude"),
    },
  };
  const base_ref = params.get("base_ref");
  const head_ref = params.get("head_ref");
  if (base_ref) request.base_ref = base_ref;
  if (head_ref) request.head_ref = head_ref;
  if (params.get("compare_from_merge_base") === "true") request.compare_from_merge_base = true;
  return request;
}
