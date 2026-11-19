// app_core over the web server's JSON API (../../../../axum/src/). An
// error answer, `{"error": "..."}`, is thrown with its message and its
// `status`, as a FormError with the fields' messages when it has
// `"errors"`. An answer that is not JSON is thrown too, naming the
// request, so a server without the route never passes for one with it.
import { FormError } from "./form_error.js";
import { sha256Of } from "./checksums.js";
import { putFile } from "./uploads.js";

/**
 * @param {string} method
 * @param {string} path
 * @param {unknown} [body] sent as JSON
 */
async function send(method, path, body) {
  const response = await fetch(path, {
    method,
    headers: { Accept: "application/json", ...(body === undefined ? {} : { "Content-Type": "application/json" }) },
    body: body === undefined ? undefined : JSON.stringify(body),
  });
  if (!response.ok) throw await errorOf(response);
  if (response.status === 204) return undefined;
  const contentType = response.headers.get("Content-Type") ?? "";
  if (!contentType.includes("application/json")) {
    throw new Error(`The server answered ${method} ${path} with ${contentType || "nothing"}, not JSON`);
  }
  return response.json();
}

/** @param {Response} response */
async function errorOf(response) {
  return Object.assign(await errorWithMessageOf(response), { status: response.status });
}

/** @param {Response} response */
async function errorWithMessageOf(response) {
  try {
    const { error, errors } = await response.json();
    if (errors) return new FormError(error, errors);
    if (typeof error === "string") return new Error(error);
  } catch {
    // Not the API's JSON: fall back to the status
  }
  return new Error(`${response.status} ${response.statusText}`);
}

/** @param {string} id */
const postPath = (id) => `/api/posts/${encodeURIComponent(id)}`;

/** The user signed in, whom the server takes the request to be from; null when nobody is. */
export async function currentUserGet() {
  try {
    return await send("GET", "/api/current-user");
  } catch (problem) {
    // 410: the account of this browser's cookie was deleted
    if ([401, 410].includes(/** @type {{status?: number}} */ (problem).status ?? 0)) return null;
    throw problem;
  }
}

/**
 * The signed-in user's new name.
 * @param {string} name
 */
export function currentUserUpdate(name) {
  return send("PUT", "/api/current-user", { name });
}

/** The community the person works in, kept by this browser; null when they are in none. */
export function currentCommunityGet() {
  return send("GET", "/api/current-community");
}

/** @param {string} communityId one the person is a member of */
export function currentCommunityUpdate(communityId) {
  return send("PUT", "/api/current-community", { community_id: communityId });
}

/**
 * Where to PUT the picture the person picked, once the server takes it.
 * @param {string} contentType
 * @param {number} size in bytes
 * @returns {Promise<{upload_url: string, content_type: string, key: string}>}
 */
export function avatarsCreateUpload(contentType, size) {
  return send("POST", "/api/avatars/uploads", { content_type: contentType, size });
}

/**
 * Makes the uploaded file the user's picture; answers the user.
 * @param {string} key
 */
export function avatarsUpdate(key) {
  return send("PUT", "/api/avatars", { key });
}

// A sign-up or a sign-in emails a code and answers `{pending}`. The code
// answers the user and sets the cookie the browser sends from then on, or,
// with a device's challenge, `{code}` for the device and no cookie; or
// `{next, pending}` when a password is asked for first ("password") or a
// new account may make one ("new_password"), which then answers as the
// code would have. A sign-out clears the cookie.

/**
 * What finishes a sign-in: the session, or a device's code.
 * @typedef {{user: {id: string, name: string}, email: string, token: string} | {code: string}} SignInFinished
 */

/**
 * @param {string} email
 * @param {string} name what the pages show the person by
 * @returns {Promise<{pending: string}>}
 */
export function accountsSignUp(email, name) {
  return send("POST", "/api/accounts/sign-up", { email, name });
}

/**
 * @param {string} email
 * @returns {Promise<{pending: string}>}
 */
export function accountsSignIn(email) {
  return send("POST", "/api/accounts/sign-in", { email });
}

/**
 * @param {string} pending
 * @param {string} code the six digits emailed
 * @param {string} [challenge] the device's, when it opened this page
 * @param {boolean} [keepSignedIn] whether the browser stays signed in after it closes
 * @returns {Promise<SignInFinished | {next: "password" | "new_password", pending: string, can_reset_password: boolean}>}
 */
export function accountsCode(pending, code, challenge, keepSignedIn = false) {
  return send("POST", "/api/accounts/code", { pending, code, challenge: challenge || null, keep_signed_in: keepSignedIn });
}

/**
 * The account's password, after the code.
 * @param {string} pending the one the code answered
 * @param {string} password
 * @param {string} [challenge]
 * @param {boolean} [keepSignedIn]
 * @returns {Promise<SignInFinished>}
 */
export function accountsSignInPassword(pending, password, challenge, keepSignedIn = false) {
  return send("POST", "/api/accounts/sign-in/password", { pending, password, challenge: challenge || null, keep_signed_in: keepSignedIn });
}

/**
 * A new password in place of a forgotten one, after the code; refused
 * when the app does not let one be reset.
 * @param {string} pending the one the code answered
 * @param {string} password
 * @param {string} [challenge]
 * @param {boolean} [keepSignedIn]
 * @returns {Promise<SignInFinished>}
 */
export function accountsSignInResetPassword(pending, password, challenge, keepSignedIn = false) {
  return send("POST", "/api/accounts/sign-in/reset-password", { pending, password, challenge: challenge || null, keep_signed_in: keepSignedIn });
}

/**
 * The password a new account makes after its first code, or null to skip.
 * @param {string} pending the one the code answered
 * @param {string | null} password
 * @param {string} [challenge]
 * @param {boolean} [keepSignedIn]
 * @returns {Promise<SignInFinished>}
 */
export function accountsSignInNewPassword(pending, password, challenge, keepSignedIn = false) {
  return send("POST", "/api/accounts/sign-in/new-password", { pending, password, challenge: challenge || null, keep_signed_in: keepSignedIn });
}

/**
 * What the signed-in person's account asks for after the emailed code.
 * @returns {Promise<Array<"password">>}
 */
export function accountsSecondFactors() {
  return send("GET", "/api/accounts/second-factors");
}

/**
 * The signed-in person's password: their first, or a new one in place of
 * the current one.
 * @param {string | null} current
 * @param {string} password
 */
export function accountsPasswordUpdate(current, password) {
  return send("PUT", "/api/accounts/password", { current, password });
}

/**
 * A new code emailed for the pending sign-in; the old one stops working.
 * @param {string} pending
 */
export function accountsCodeResend(pending) {
  return send("POST", "/api/accounts/code/resend", { pending });
}

// The browser is where a sign-in happens; nothing waits on it.

/** @param {string} [_email] */
export function accountsSignInStart(_email) {
  return Promise.reject(new Error("the web signs in on its own page"));
}

export function accountsSignInPending() {
  return Promise.resolve(null);
}

/** @param {string} _code */
export function accountsSignInFinish(_code) {
  return Promise.reject(new Error("the web signs in on its own page"));
}

export function accountsSignInCancel() {
  return Promise.resolve(null);
}

export function accountsSignOut() {
  return send("DELETE", "/api/accounts/sign-out");
}

// The web is one browser session: sign in and sign out, no accounts kept
// on the device. The accounts page is never linked.

export const hasDeviceAccounts = false;

export function deviceAccountsList() {
  return Promise.resolve([]);
}

/** @param {string} _userId */
export function deviceAccountsSwitch(_userId) {
  return Promise.reject(new Error("no accounts are kept on the web"));
}

export function deviceAccountsDeactivate() {
  return Promise.resolve(null);
}

/** @param {string} _userId @param {boolean} _losingChanges */
export function deviceAccountsRemove(_userId, _losingChanges) {
  return Promise.reject(new Error("no accounts are kept on the web"));
}

// A trial user: someone using the app on the web without signing up,
// for a day, see docs/design/accounts-local-and-trial-users.md.

/**
 * Whether "Try it now" would make a trial user: the server has trials
 * on and room for one.
 * @returns {Promise<boolean>}
 */
export async function accountsTrialGet() {
  return (await send("GET", "/api/accounts/trial")).available;
}

/**
 * "Try it now": makes a trial user and signs this browser in as them.
 * A browser already signed in stays as it is.
 */
export function accountsTrialCreate() {
  return send("POST", "/api/accounts/trial");
}

/** What is for sale, plans first, each with the prices it sells at. */
export function accountsProductsList() {
  return send("GET", "/api/accounts/products");
}

/**
 * One product for sale, with its prices.
 * @param {string} id
 */
export function accountsProductsGet(id) {
  return send("GET", `/api/accounts/products/${encodeURIComponent(id)}`);
}

/**
 * A checkout for the price, answering `{checkout, url}`: Stripe's page to pay on.
 * @param {string} priceId
 */
export function accountsCheckoutsCreate(priceId) {
  return send("POST", "/api/accounts/checkouts", { price_id: priceId });
}

/**
 * A checkout, asked of Stripe while it may still change.
 * @param {string} id
 */
export function accountsCheckoutsGet(id) {
  return send("GET", `/api/accounts/checkouts/${encodeURIComponent(id)}`);
}

/** The signed-in person's one-time purchases, newest first. */
export function accountsPurchasesList() {
  return send("GET", "/api/accounts/purchases");
}

/** @param {string} id */
export function accountsPurchasesGet(id) {
  return send("GET", `/api/accounts/purchases/${encodeURIComponent(id)}`);
}

/** The signed-in person's plans, newest first, ended ones too. */
export function accountsSubscriptionsList() {
  return send("GET", "/api/accounts/subscriptions");
}

/** @param {string} id */
export function accountsSubscriptionsGet(id) {
  return send("GET", `/api/accounts/subscriptions/${encodeURIComponent(id)}`);
}

/**
 * A change to a plan, answering the plan as it is now.
 * @param {string} id
 * @param {{action: "switch", price_id: string} | {action: "switch_undo" | "cancel" | "resume"}} change
 */
export function accountsSubscriptionsUpdate(id, change) {
  return send("PUT", `/api/accounts/subscriptions/${encodeURIComponent(id)}`, change);
}

/**
 * Stripe's Customer Portal for the plan's card and invoices, answering `{url}`.
 * @param {string} id
 */
export function accountsSubscriptionsPortalCreate(id) {
  return send("POST", `/api/accounts/subscriptions/${encodeURIComponent(id)}/portal`);
}

// Local users are a device's; the web has trial users instead.

export function localUsersCanCreate() {
  return Promise.resolve(false);
}

export function localUsersCreate() {
  return Promise.reject(new Error("only the app has local users"));
}

export function localUsersMove() {
  return Promise.reject(new Error("only the app has local users"));
}

export function localUsersKeepSeparate() {
  return Promise.reject(new Error("only the app has local users"));
}

export function postsList() {
  return send("GET", "/api/posts");
}

/** @param {string} id */
export function postsGet(id) {
  return send("GET", postPath(id));
}

/**
 * @param {string} body
 * @param {string[]} communityIds the communities the post is in
 */
export function postsCreate(body, communityIds) {
  return send("POST", "/api/posts", { body, community_ids: communityIds });
}

/**
 * @param {string} id
 * @param {string} body
 * @param {string[]} communityIds the communities the post is in, exactly
 */
export function postsUpdate(id, body, communityIds) {
  return send("PUT", postPath(id), { body, community_ids: communityIds });
}

/** @param {string} id */
export function postsDelete(id) {
  return send("DELETE", postPath(id));
}

/** @param {string} id */
const filePath = (id) => `/api/files/${encodeURIComponent(id)}`;

/** The files the user sees, each `{file, creator, communities, shown_as, state, error}`. */
export function filesList() {
  return send("GET", "/api/files");
}

/**
 * The files the user sees with any of these checksums, up to 500 at once.
 * @param {string[]} sha256s
 */
export function filesListDuplicates(sha256s) {
  return send("POST", "/api/files/duplicates", { sha256s });
}

/** @param {string} id */
export function filesGet(id) {
  return send("GET", filePath(id));
}

/** How many files a page creates at once: each waits on its upload. */
export const filesCreateAtOnce = 3;

/**
 * Creates the file the person picked: its bytes go straight to object
 * storage, checked against their SHA-256, then the server writes its
 * row; answers the file.
 * @param {File} file
 * @param {string} name
 * @param {string[]} communityIds the communities the file is in, one at least
 * @param {(fraction: number) => void} [onProgress] from 0 to 1
 * @param {string} [sha256] the file's, when the page already has it
 */
export async function filesCreate(file, name, communityIds, onProgress, sha256) {
  sha256 ??= await sha256Of(file);
  const upload = await send("POST", "/api/files/upload", { content_type: file.type, size: file.size, sha256 });
  if (!upload.uploaded) await putFile(upload, file, onProgress);
  return send("POST", "/api/files", {
    id: upload.id,
    name,
    content_type: upload.content_type,
    size: file.size,
    sha256,
    community_ids: communityIds,
  });
}


/**
 * @param {string} id
 * @param {string} name
 * @param {string[]} communityIds its communities among the user's, exactly
 */
export function filesUpdate(id, name, communityIds) {
  return send("PUT", filePath(id), { name, community_ids: communityIds });
}

/** @param {string} id */
export function filesDelete(id) {
  return send("DELETE", filePath(id));
}

/**
 * Where a page reads the file's bytes from, for an `<img>` or a frame.
 * @param {string} id
 */
export function filesContentUrl(id) {
  return `${filePath(id)}/content`;
}

/** A browser keeps no copies of its own: null. */
export function filesCacheGet() {
  return Promise.resolve(null);
}

export function filesCacheDelete() {
  return Promise.resolve(null);
}

/** Whether this browser writes images to the clipboard. */
export function filesCopySupported() {
  return Promise.resolve(typeof ClipboardItem !== "undefined" && Boolean(navigator.clipboard?.write));
}

/**
 * Copies the image to the clipboard as a PNG, the one image type
 * browsers write, so an animated GIF is copied as its first frame. The
 * write starts before any await, inside the click, as Safari requires;
 * the bytes come with no cache, since a copy the page's <img> left in
 * the cache lacks the bucket's CORS headers this read needs.
 * @param {string} id
 */
export function filesCopy(id) {
  const png = fetch(`${filePath(id)}/content`, { cache: "no-store" }).then(async (response) => {
    if (!response.ok) throw new Error(`Could not read the image (${response.status})`);
    return asPng(await response.blob());
  });
  return navigator.clipboard.write([new ClipboardItem({ "image/png": png })]);
}

/**
 * @param {Blob} image
 * @returns {Promise<Blob>}
 */
async function asPng(image) {
  if (image.type === "image/png") return image;
  const bitmap = await createImageBitmap(image);
  const canvas = document.createElement("canvas");
  canvas.width = bitmap.width;
  canvas.height = bitmap.height;
  canvas.getContext("2d")?.drawImage(bitmap, 0, 0);
  bitmap.close();
  return new Promise((resolve, reject) =>
    canvas.toBlob((png) => (png ? resolve(png) : reject(new Error("Could not copy the image"))), "image/png"),
  );
}

export function filesDownloadSupported() {
  return Promise.resolve(true);
}

/**
 * Has the browser save the file under its name.
 * @param {string} id
 * @returns {Promise<string | null>} where it was saved, when known
 */
export function filesDownload(id) {
  window.location.assign(`${filePath(id)}/content?download=1`);
  return Promise.resolve(null);
}

/** Every community, each with the user's membership when they are in it. */
export function communitiesList() {
  return send("GET", "/api/communities");
}

/** @param {string} id */
const communityPath = (id) => `/api/communities/${encodeURIComponent(id)}`;

/** One community, `{community, membership}`. @param {string} id */
export function communitiesGet(id) {
  return send("GET", communityPath(id));
}

/** @param {string} name */
export function communitiesCreate(name) {
  return send("POST", "/api/communities", { name });
}

/**
 * @param {string} id
 * @param {string} name
 */
export function communitiesUpdate(id, name) {
  return send("PUT", communityPath(id), { name });
}

/** @param {string} id */
export function communitiesDelete(id) {
  return send("DELETE", communityPath(id));
}

/** @param {string} communityId */
export function membershipsJoin(communityId) {
  return send("PUT", `/api/memberships/${encodeURIComponent(communityId)}`);
}

/** @param {string} communityId */
export function membershipsLeave(communityId) {
  return send("DELETE", `/api/memberships/${encodeURIComponent(communityId)}`);
}

// The web has nothing to sync: it is the server. The nav shows no sync
// status, and the conflicts page shows none.

export function syncRun() {
  return Promise.resolve(null);
}

export function syncStatus() {
  return Promise.resolve(null);
}

/** @param {boolean} _offline */
export function syncSetOffline(_offline) {
  return Promise.resolve(null);
}

/**
 * @param {(outcome: unknown) => void} _callback
 * @returns {() => void} stops listening
 */
export function onSynced(_callback) {
  return () => {};
}

export function conflictsList() {
  return Promise.resolve([]);
}

export function pendingChangesList() {
  return Promise.resolve([]);
}

/** @param {string} _id */
export function conflictsReapply(_id) {
  return Promise.reject(new Error("nothing to reapply on the web"));
}

/** @param {string} _id */
export function conflictsTryAgain(_id) {
  return Promise.reject(new Error("nothing to try again on the web"));
}

/** @param {string} _id */
export function conflictsLetGo(_id) {
  return Promise.reject(new Error("nothing to let go on the web"));
}

// The quick picker is the desktop app's: a browser has none.

export function quickPickerList() {
  return Promise.reject(new Error("the quick picker is the desktop app's"));
}

/** @param {string} _kind @param {string} _id */
export function quickPickerPaste(_kind, _id) {
  return Promise.reject(new Error("the quick picker is the desktop app's"));
}

export function quickPickerClose() {
  return Promise.resolve();
}

export function quickPickerOpenApp() {
  return Promise.reject(new Error("the quick picker is the desktop app's"));
}

export function quickPickerAccessibilityRequest() {
  return Promise.resolve(false);
}

/**
 * @param {(opened: {accessibility_trusted: boolean}) => void} _callback
 * @returns {() => void} stops listening
 */
export function onQuickPickerOpened(_callback) {
  return () => {};
}

/**
 * Opens a public page, `terms` or `privacy`, in a new tab.
 * @param {"terms" | "privacy"} page
 */
export function publicPagesOpen(page) {
  window.open(`/${page}`, "_blank", "noopener");
  return Promise.resolve();
}

/** Mails the signed-in person the code that confirms deleting their account. */
export function accountsDeletionsCodeCreate() {
  return send("POST", "/api/accounts/deletions/code");
}

/**
 * Deletes the signed-in person's account with the mailed code, none for a trial.
 * @param {string | null} code
 */
export function accountsDeletionsCreate(code) {
  return send("POST", "/api/accounts/deletions", { code });
}

/** The store this app sells through: on the web, Stripe. */
export function accountsStoreGet() {
  return Promise.resolve("stripe");
}

/** A phone store's sheet: never on the web. */
export function accountsStoreTransactionsCreate(_priceId) {
  return Promise.reject(new Error("On the web, plans and products are bought through Stripe"));
}

export function accountsStoreTransactionsRestore() {
  return Promise.reject(new Error("Restoring purchases is in the iPhone app"));
}

export function accountsSubscriptionsManageOpen() {
  return Promise.reject(new Error("A plan from a phone's store is managed on the phone"));
}
