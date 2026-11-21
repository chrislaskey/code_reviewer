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
export const ACCOUNTS_SIGN_UP_PATH = "/accounts/sign-up";
export const ACCOUNTS_SIGN_IN_PATH = "/accounts/sign-in";
export const ACCOUNTS_TRIAL_PATH = "/accounts/trial";
export const ACCOUNTS_SETTINGS_PATH = "/accounts/settings";
export const ACCOUNTS_PASSWORD_EDIT_PATH = "/accounts/password/edit";
export const ACCOUNTS_DELETIONS_NEW_PATH = "/accounts/deletions/new";
export const ACCOUNTS_PRODUCTS_INDEX_PATH = "/accounts/products";
export const ACCOUNTS_PRODUCTS_SHOW_PATH = "/accounts/products/:id";
export const ACCOUNTS_CHECKOUTS_SHOW_PATH = "/accounts/checkouts/:id";
export const ACCOUNTS_BACK_TO_APP_PATH = "/accounts/back-to-app";
export const ACCOUNTS_PURCHASES_INDEX_PATH = "/accounts/purchases";
export const ACCOUNTS_PURCHASES_SHOW_PATH = "/accounts/purchases/:id";
export const ACCOUNTS_SUBSCRIPTIONS_INDEX_PATH = "/accounts/subscriptions";
export const ACCOUNTS_SUBSCRIPTIONS_SHOW_PATH = "/accounts/subscriptions/:id";
export const POSTS_INDEX_PATH = "/posts";
export const POSTS_NEW_PATH = "/posts/new";
export const POSTS_SHOW_PATH = "/posts/:id";
export const POSTS_EDIT_PATH = "/posts/:id/edit";
export const FILES_INDEX_PATH = "/files";
export const FILES_NEW_PATH = "/files/new";
export const FILES_SHOW_PATH = "/files/:id";
export const FILES_EDIT_PATH = "/files/:id/edit";
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
  ACCOUNTS_SIGN_UP_PATH,
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

/**
 * The accounts on this device, inside the app only.
 * @param {{accountDeleted?: boolean}} [options] to say an account was just deleted
 */
export function accountsIndexUrl({ accountDeleted = false } = {}) {
  return accountDeleted ? `${ACCOUNTS_INDEX_PATH}?account_deleted=1` : ACCOUNTS_INDEX_PATH;
}

// The account pages carry two things in their query, which every link
// between them keeps unless the caller says otherwise: a device's
// challenge, when it opened the page in the browser to sign in, where
// to return the code to, the desktop app's port or the iPhone app's
// scheme, and the email to fill in. So the nav's Sign in and Sign up, and the pages'
// own links, all stay a device's sign-in.

/**
 * @param {string} [email] to fill in, else the current address's
 * @param {string} [challenge] a device's, else the current address's
 */
export function accountsSignUpUrl(email = currentQuery("email"), challenge = currentQuery("challenge")) {
  return withQuery(ACCOUNTS_SIGN_UP_PATH, { email, challenge, ...appReturnQuery() });
}

/**
 * @param {string} [email] to fill in, for an account signed out on this device, else the current address's
 * @param {string} [challenge] a device's, else the current address's
 */
export function accountsSignInUrl(email = currentQuery("email"), challenge = currentQuery("challenge")) {
  return withQuery(ACCOUNTS_SIGN_IN_PATH, { email, challenge, ...appReturnQuery() });
}

/** The iPhone app's scheme, its bundle identifier in tauri.conf.json. */
const APP_URL_SCHEME = "com.starterpack.app";

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

export function postsIndexUrl() {
  return POSTS_INDEX_PATH;
}

export function postsNewUrl() {
  return POSTS_NEW_PATH;
}

/** @param {string} id */
export function postsShowUrl(id) {
  return POSTS_SHOW_PATH.replace(":id", encodeURIComponent(id));
}

/** @param {string} id */
export function postsEditUrl(id) {
  return POSTS_EDIT_PATH.replace(":id", encodeURIComponent(id));
}

export function filesIndexUrl() {
  return FILES_INDEX_PATH;
}

export function filesNewUrl() {
  return FILES_NEW_PATH;
}

/** @param {string} id */
export function filesShowUrl(id) {
  return FILES_SHOW_PATH.replace(":id", encodeURIComponent(id));
}

/** @param {string} id */
export function filesEditUrl(id) {
  return FILES_EDIT_PATH.replace(":id", encodeURIComponent(id));
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
export function accountsSettingsUrl() {
  return ACCOUNTS_SETTINGS_PATH;
}

/** Where the signed-in person makes or changes their password. */
export function accountsPasswordEditUrl() {
  return ACCOUNTS_PASSWORD_EDIT_PATH;
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
