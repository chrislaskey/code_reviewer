// Every page's address, so pages link to each other through here and never
// spell out a path themselves. The paths are the router's too (router.js),
// so they are named once, here. A page takes what it shows from its path,
// the way a web server would read it, so a page is a plain link, from
// another page or from a bookmark.

// The public website's home page on the web (src/public); the app has no
// page there, so its router sends `/` to the app's home.
export const PUBLIC_HOME_PATH = "/";
export const HOME_PATH = "/home";
export const ACCOUNTS_INDEX_PATH = "/accounts";
export const ACCOUNTS_SIGN_IN_PATH = "/accounts/sign-in";
export const ACCOUNTS_TRIAL_PATH = "/accounts/trial";
export const ACCOUNTS_CURRENT_SHOW_PATH = "/accounts/current";
export const ACCOUNTS_CURRENT_PASSWORD_EDIT_PATH = "/accounts/current/password/edit";
export const ACCOUNTS_DELETIONS_NEW_PATH = "/accounts/deletions/new";
export const ACCOUNTS_PRODUCTS_INDEX_PATH = "/accounts/products";
export const ACCOUNTS_PRODUCTS_SHOW_PATH = "/accounts/products/:id";
export const ACCOUNTS_CHECKOUTS_SHOW_PATH = "/accounts/checkouts/:id";
export const ACCOUNTS_BACK_TO_APP_PATH = "/accounts/back-to-app";
export const ACCOUNTS_PURCHASES_INDEX_PATH = "/accounts/purchases";
export const ACCOUNTS_PURCHASES_SHOW_PATH = "/accounts/purchases/:id";
export const ACCOUNTS_SUBSCRIPTIONS_INDEX_PATH = "/accounts/subscriptions";
export const ACCOUNTS_SUBSCRIPTIONS_SHOW_PATH = "/accounts/subscriptions/:id";
export const COMMUNITIES_INDEX_PATH = "/communities";
export const COMMUNITIES_NEW_PATH = "/communities/new";
export const COMMUNITIES_SHOW_PATH = "/communities/:id";
export const COMMUNITIES_EDIT_PATH = "/communities/:id/edit";
export const CONFLICTS_INDEX_PATH = "/conflicts";
export const PROFILE_EDIT_PATH = "/profile/edit";
// The other apps, each under its own path, see docs/design/multi-app.md.
export const SUPPORT_INDEX_PATH = "/support";
export const FORUM_INDEX_PATH = "/forum";
// The ops app is an entry of its own (ops.html), so a link to it loads
// the page in full: it is never one of this router's paths.
export const OPS_HOME_PATH = "/ops";

/** The pages anyone sees; every other page needs someone signed in. */
export const OPEN_PATHS = [
  HOME_PATH,
  ACCOUNTS_INDEX_PATH,
  ACCOUNTS_SIGN_IN_PATH,
  ACCOUNTS_TRIAL_PATH,
  ACCOUNTS_BACK_TO_APP_PATH,
];

export function homeUrl() {
  return HOME_PATH;
}

/** "Try it now", on the web only: the app for a day with no sign-up. */
export function accountsTrialUrl() {
  return ACCOUNTS_TRIAL_PATH;
}

/** The accounts on this device, inside the app only. */
export function accountsIndexUrl() {
  return ACCOUNTS_INDEX_PATH;
}

// The account pages carry two things in their query, which every link
// between them keeps unless the caller says otherwise: a device's
// challenge, when it opened the page in the browser to sign in, where
// to return the code to, the desktop app's port or the iPhone app's
// scheme, and the email to fill in. So the nav's Sign in, and the pages'
// own links, all stay a device's sign-in.

/**
 * @param {string} [email] to fill in, for an account signed out on this device, else the current address's
 * @param {string} [challenge] a device's, else the current address's
 */
export function accountsSignInUrl(email = currentQuery("email"), challenge = currentQuery("challenge")) {
  return withQuery(ACCOUNTS_SIGN_IN_PATH, { email, challenge, ...appReturnQuery() });
}

/** The iPhone app's scheme, its bundle identifier: `APP_IDENTIFIER`, which a test keeps it equal to. */
const APP_URL_SCHEME = "com.chrislaskey.codereviewer";

/**
 * Where the browser returns a device code to the app that started the
 * sign-in: the desktop app's listener on this computer, only ever a
 * loopback address, or the iPhone app's own scheme, which closes its
 * sign-in sheet. "" when neither is asked for, and the code is pasted.
 * @param {{appPort?: string, appScheme?: string}} appReturn from the page's query
 * @param {string} code
 */
export function accountsAppReturnUrl({ appPort = "", appScheme = "" }, code) {
  const query = `?code=${encodeURIComponent(code)}`;
  if (appScheme === APP_URL_SCHEME) return `${APP_URL_SCHEME}://accounts/sign-in${query}`;
  const port = Number(appPort);
  if (!/^\d+$/.test(appPort) || port < 1 || port > 65535) return "";
  return `http://127.0.0.1:${port}/${query}`;
}

/** The current address's way back to the app, for the links between the pages. */
function appReturnQuery() {
  return { app_port: currentQuery("app_port"), app_scheme: currentQuery("app_scheme") };
}

/**
 * A value in the current address's query, or "" when absent or when
 * there is no window yet.
 * @param {string} name
 */
function currentQuery(name) {
  if (typeof window === "undefined") return "";
  return new URLSearchParams(window.location.search).get(name) ?? "";
}

/**
 * @param {string} path
 * @param {Record<string, string | undefined>} values the query, without the empty ones
 */
function withQuery(path, values) {
  const query = new URLSearchParams();
  for (const [name, value] of Object.entries(values)) {
    if (value) query.set(name, value);
  }
  const string = query.toString();
  return string ? `${path}?${string}` : path;
}

export function communitiesIndexUrl() {
  return COMMUNITIES_INDEX_PATH;
}

export function communitiesNewUrl() {
  return COMMUNITIES_NEW_PATH;
}

/** @param {string} id */
export function communitiesShowUrl(id) {
  return COMMUNITIES_SHOW_PATH.replace(":id", encodeURIComponent(id));
}

/** @param {string} id */
export function communitiesEditUrl(id) {
  return COMMUNITIES_EDIT_PATH.replace(":id", encodeURIComponent(id));
}

export function conflictsIndexUrl() {
  return CONFLICTS_INDEX_PATH;
}

export function profileEditUrl() {
  return PROFILE_EDIT_PATH;
}

/** The signed-in person's account: their email, and later their plan. */
export function accountsCurrentShowUrl() {
  return ACCOUNTS_CURRENT_SHOW_PATH;
}

/** Where the signed-in person makes or changes their password. */
export function accountsCurrentPasswordEditUrl() {
  return ACCOUNTS_CURRENT_PASSWORD_EDIT_PATH;
}

/** Where the signed-in person deletes their account. */
export function accountsDeletionsNewUrl() {
  return ACCOUNTS_DELETIONS_NEW_PATH;
}

/** What is for sale: plans, then one-time products. */
export function accountsProductsIndexUrl() {
  return ACCOUNTS_PRODUCTS_INDEX_PATH;
}

/** @param {string} id */
export function accountsProductsShowUrl(id) {
  return ACCOUNTS_PRODUCTS_SHOW_PATH.replace(":id", encodeURIComponent(id));
}

/**
 * Where Stripe's page returns to after paying, see docs/design/purchasing.md, P3.
 * @param {string} id
 */
export function accountsCheckoutsShowUrl(id) {
  return ACCOUNTS_CHECKOUTS_SHOW_PATH.replace(":id", encodeURIComponent(id));
}

/** The signed-in person's one-time purchases. */
export function accountsPurchasesIndexUrl() {
  return ACCOUNTS_PURCHASES_INDEX_PATH;
}

/** @param {string} id */
export function accountsPurchasesShowUrl(id) {
  return ACCOUNTS_PURCHASES_SHOW_PATH.replace(":id", encodeURIComponent(id));
}

/** The signed-in person's plans, the current one and the ended ones. */
export function accountsSubscriptionsIndexUrl() {
  return ACCOUNTS_SUBSCRIPTIONS_INDEX_PATH;
}

/** @param {string} id */
export function accountsSubscriptionsShowUrl(id) {
  return ACCOUNTS_SUBSCRIPTIONS_SHOW_PATH.replace(":id", encodeURIComponent(id));
}

export function supportIndexUrl() {
  return SUPPORT_INDEX_PATH;
}

export function forumIndexUrl() {
  return FORUM_INDEX_PATH;
}

/** The ops app's home, on the web, for staff. */
export function opsHomeUrl() {
  return OPS_HOME_PATH;
}

// Reviews and traces: a page takes its comparison from its query, flat and
// named, `/reviews/show?repo_path=…&base_ref=main&head_ref=cr-fix`, with a
// list as a repeated key.
export const REVIEWS_NEW_PATH = "/reviews/new";
export const REVIEWS_SHOW_PATH = "/reviews/show";
export const TRACES_NEW_PATH = "/traces/new";
export const TRACES_SHOW_PATH = "/traces/show";

/** The new review page, with its form filled in from a request when given one. */
export function reviewsNewUrl(request = null) {
  return withSearchParams(REVIEWS_NEW_PATH, request ? reviewRequestToParams(request) : null);
}

/** The show page, running a request (the core crate's `analyze_file_changes` request). */
export function reviewsShowUrl(request) {
  return withSearchParams(REVIEWS_SHOW_PATH, reviewRequestToParams(request));
}

/** The new trace page, with its form filled in from a request when given one. */
export function tracesNewUrl(request = null) {
  return withSearchParams(TRACES_NEW_PATH, request ? traceRequestToParams(request) : null);
}

/** The trace page: the tests of a comparison, and the call stack of the test in the request if it names one. */
export function tracesShowUrl(request) {
  return withSearchParams(TRACES_SHOW_PATH, traceRequestToParams(request));
}

/** A path with a query whose lists repeat their key. */
function withSearchParams(path, params) {
  const query = params?.toString() ?? "";
  return query === "" ? path : `${path}?${query}`;
}

/* -- A comparison as query parameters ------------------------------------------- */

/** Adds the core crate's AnalyzeChangesRequest fields: the repository and the two sides. */
function setComparisonParams(params, request) {
  params.set("repo_path", request.repo_path);
  if (request.base_ref) params.set("base_ref", request.base_ref);
  if (request.head_ref) params.set("head_ref", request.head_ref);
  if (request.compare_from_merge_base) params.set("compare_from_merge_base", "true");
}

/** The comparison in a page's query, or null when it names no repository. */
function comparisonFromParams(params) {
  const repo_path = params.get("repo_path");
  if (!repo_path) return null;
  const request = { repo_path };
  const base_ref = params.get("base_ref");
  const head_ref = params.get("head_ref");
  if (base_ref) request.base_ref = base_ref;
  if (head_ref) request.head_ref = head_ref;
  if (params.get("compare_from_merge_base") === "true") request.compare_from_merge_base = true;
  return request;
}

/* -- A review request as query parameters --------------------------------------- */

/** @param {object} request the core crate's AnalyzeFileChangesRequest, with snake_case fields. */
export function reviewRequestToParams(request) {
  const params = new URLSearchParams();
  setComparisonParams(params, request);
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
  const comparison = comparisonFromParams(params);
  if (comparison === null) return null;
  return {
    ...comparison,
    module_rules: { include: params.getAll("module_include"), ignore: params.getAll("module_ignore") },
    group_flags: {
      group_config_path: params.get("group_config_path") || null,
      include: params.getAll("group_include"),
      exclude: params.getAll("group_exclude"),
    },
  };
}

/* -- A trace request as query parameters ---------------------------------------- */

/**
 * @param {object} request the comparison (the core crate's AnalyzeChangesRequest fields) and, once one is
 * picked, `test`: the FunctionId of the test to walk.
 */
export function traceRequestToParams(request) {
  const params = new URLSearchParams();
  setComparisonParams(params, request);
  if (request.test) {
    params.set("test_module", request.test.module);
    params.set("test_name", request.test.name);
    params.set("test_arity", String(request.test.arity));
  }
  return params;
}

/** The request in a page's query (`location.search`), or null when it names no repository. */
export function traceRequestFromSearch(search) {
  const params = new URLSearchParams(search);
  const request = comparisonFromParams(params);
  if (request === null) return null;
  const module = params.get("test_module");
  const name = params.get("test_name");
  const arity = Number.parseInt(params.get("test_arity") ?? "", 10);
  if (module && name && Number.isInteger(arity)) request.test = { module, name, arity };
  return request;
}

// Sessions: a session's page takes its name from its path, `/sessions/cr-fix`.
export const SESSIONS_INDEX_PATH = "/sessions";
export const SESSIONS_NEW_PATH = "/sessions/new";
export const SESSIONS_SHOW_PATH = "/sessions/:name";

export function sessionsIndexUrl() {
  return SESSIONS_INDEX_PATH;
}

export function sessionsNewUrl() {
  return SESSIONS_NEW_PATH;
}

/** @param {string} name the session's full name, prefix included */
export function sessionsShowUrl(name) {
  return SESSIONS_SHOW_PATH.replace(":name", encodeURIComponent(name));
}

/**
 * The session name in a session page's path (`location.pathname`), or null when it names none.
 * @param {string} pathname
 */
export function sessionNameFromPath(pathname) {
  const prefix = `${SESSIONS_INDEX_PATH}/`;
  if (!pathname.startsWith(prefix)) return null;
  const name = decodeURIComponent(pathname.slice(prefix.length));
  return name === "" ? null : name;
}
