// app_core over the Tauri app's commands (../../../../tauri/src/).
import { convertFileSrc, invoke as tauriInvoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { FormError } from "./form_error.js";

/** The user signed in on this device; null when nobody is. */
export function currentUserGet() {
  return invoke("current_user_get");
}

/**
 * The signed-in user's new name, written on this device and synced when it next can.
 * @param {string} name
 */
export function currentUserUpdate(name) {
  return invoke("current_user_update", { name });
}

/** The community the person works in on this device; null when they are in none. */
export function currentCommunityGet() {
  return invoke("current_community_get");
}

/** @param {string} communityId one the person is a member of */
export function currentCommunityUpdate(communityId) {
  return invoke("current_community_update", { communityId });
}

/**
 * A command, whose error, `{error, errors}`, is thrown as the browser's
 * would be: a FormError when fields were wrong, else an Error.
 * @param {string} command
 * @param {import("@tauri-apps/api/core").InvokeArgs} [args]
 * @param {import("@tauri-apps/api/core").InvokeOptions} [options]
 */
async function invoke(command, args, options) {
  try {
    return await tauriInvoke(command, args, options);
  } catch (problem) {
    throw errorOf(problem);
  }
}

/**
 * A command's `{error, errors}` as an Error; anything else as it is.
 * @param {unknown} problem
 */
function errorOf(problem) {
  if (problem && typeof problem === "object" && "error" in problem) {
    const { error, errors } = /** @type {{error: string, errors?: Record<string, string[]>}} */ (problem);
    return errors ? new FormError(error, errors) : new Error(error);
  }
  return problem;
}

/**
 * Where to PUT the picture the person picked, asked of the web server.
 * @param {string} contentType
 * @param {number} size in bytes
 * @returns {Promise<{upload_url: string, content_type: string, key: string}>}
 */
export function avatarsUploadsCreate(contentType, size) {
  return invoke("avatars_uploads_create", { contentType, size });
}

/**
 * Makes the uploaded file the user's picture, on the web server, and
 * here at once; answers the user.
 * @param {string} key
 */
export function avatarsUpdate(key) {
  return invoke("avatars_update", { key });
}

// The app asks for no password at a sign-in: it happens in the system
// browser, on the web's sign-in page, which ends by showing a code the
// person pastes into the app, see accountsSignInStart.

/** @param {string} _email */
export function accountsSignUp(_email) {
  return Promise.reject(new Error("in the app, sign in from the accounts page"));
}

/** @param {string} _email */
export function accountsSignIn(_email) {
  return Promise.reject(new Error("in the app, sign in from the accounts page"));
}

/** @param {string} _pending @param {string} _code @param {string} [_challenge] */
export function accountsCode(_pending, _code, _challenge) {
  return Promise.reject(new Error("in the app, sign in from the accounts page"));
}

/** @param {string} _pending @param {string} _password @param {string} [_challenge] */
export function accountsSignInPassword(_pending, _password, _challenge) {
  return Promise.reject(new Error("in the app, sign in from the accounts page"));
}

/** @param {string} _pending @param {string} _password @param {string} [_challenge] */
export function accountsSignInResetPassword(_pending, _password, _challenge) {
  return Promise.reject(new Error("in the app, sign in from the accounts page"));
}

/** @param {string} _pending @param {string | null} _password @param {string} [_challenge] */
export function accountsSignInNewPassword(_pending, _password, _challenge) {
  return Promise.reject(new Error("in the app, sign in from the accounts page"));
}

/**
 * What the signed-in person's account asks for after the emailed code,
 * asked of the server, so only when online.
 * @returns {Promise<Array<"password">>}
 */
export function accountsSecondFactors() {
  return invoke("accounts_second_factors");
}

/**
 * The signed-in person's password, kept by the server, so only when online.
 * @param {string | null} current
 * @param {string} password
 */
export function accountsPasswordUpdate(current, password) {
  return invoke("accounts_password_update", { current, password });
}

/** @param {string} _pending */
export function accountsCodeResend(_pending) {
  return Promise.reject(new Error("in the app, sign in from the accounts page"));
}

/**
 * Opens the browser on the web's sign-in page, with the email filled in
 * for an account signed out on this device, and waits for its code;
 * on an iPhone the page opens in the sign-in sheet over the app,
 * answered `{in_sheet: true}`, which returns the code itself.
 * @param {string} [email]
 * @param {string} [serverId] the server to sign in to, the default one when none
 * @returns {Promise<{in_sheet: boolean}>}
 */
export function accountsSignInStart(email, serverId) {
  return invoke("accounts_sign_in_start", { email: email || null, serverId: serverId || null });
}

/** The sign-in waiting on the browser, `{email, started_at}`, or null. */
export function accountsSignInPending() {
  return invoke("accounts_sign_in_pending");
}

/**
 * Exchanges the code the browser showed for the session, which becomes
 * the active account: `{outcome: "signed_in", user}`. On a device with a
 * local user, and an account new to it, nothing is done yet and it
 * answers `{outcome: "move_offered", email, items}`: the person chooses
 * localUsersMove or localUsersKeepSeparate.
 * @param {string} code
 * @returns {Promise<{outcome: "signed_in", user: object} | {outcome: "move_offered", email: string, items: number}>}
 */
export function accountsSignInFinish(code) {
  return invoke("accounts_sign_in_finish", { code });
}

export function accountsSignInCancel() {
  return invoke("accounts_sign_in_cancel");
}

/**
 * Calls back when the code came back to the app by itself, from the
 * desktop's browser or the iPhone's sign-in sheet, and the sign-in
 * finished: `{result: "finished", finished}`, with what
 * accountsSignInFinish answers, or `{result: "failed", error}`; or
 * `{result: "cancelled"}` when the person closed the iPhone's sheet.
 * Failed and cancelled end the sheet's sign-in.
 * @param {(returned: {result: "finished", finished: any} | {result: "failed", error: string} | {result: "cancelled"}) => void} callback
 * @returns {() => void} stops listening
 */
export function onAccountsSignInReturned(callback) {
  const stopping = listen("accounts-sign-in-returned", (event) => callback(/** @type {any} */ (event.payload)));
  return () => {
    stopping.then((stop) => stop());
  };
}

/**
 * Syncs what waits first, then signs out and closes the account, whose
 * data stays on the device for the next sign-in.
 */
export function accountsSignOut() {
  return invoke("accounts_sign_out");
}

/** The app keeps the accounts signed in on this device, see deviceAccountsList. */
export const hasDeviceAccounts = true;

export function deviceAccountsList() {
  return invoke("device_accounts_list");
}

/**
 * Makes a signed-in account the active one, which then syncs.
 * @param {string} id the account's id on this device
 */
export function deviceAccountsSwitch(id) {
  return invoke("device_accounts_switch", { id });
}

/** Closes the active account, which stays signed in, before adding another. */
export function deviceAccountsDeactivate() {
  return invoke("device_accounts_deactivate");
}

/**
 * Removes an account and its data from this device. Answers
 * `{outcome: "removed"}`, or, when a change would be lost and the person
 * has not chosen to lose it, `{outcome: "changes_would_be_lost", pending,
 * conflicts}` with nothing done.
 * @param {string} id the account's id on this device
 * @param {boolean} losingChanges
 */
export function deviceAccountsRemove(id, losingChanges) {
  return invoke("device_accounts_remove", { id, losingChanges });
}

/**
 * Which servers other than the build's default the app may sign in to:
 * "none", "listed_only" or "any".
 * @returns {Promise<"none" | "listed_only" | "any">}
 */
export function multiServerGet() {
  return invoke("multi_server_get");
}

/**
 * Keeps a server checked with serversGet, checked again, for a sign-in
 * to start there: `{id, url}`.
 * @param {string} url
 */
export function deviceServersCreate(url) {
  return invoke("device_servers_create", { url });
}

/**
 * Checks a server other than the build's default, before signing in
 * there: `{url, name, display_name, listing}`, the URL as its origin and
 * `listing` one of "default", "listed", "unlisted" or "unchecked"; refused
 * for a server of another app or protocol version.
 * @param {string} url
 */
export function serversGet(url) {
  return invoke("servers_get", { url });
}

// Trial users are the web's; a device has a local user instead.

export function accountsTrialGet() {
  return Promise.resolve(false);
}

export function accountsTrialCreate() {
  return Promise.reject(new Error("only the web has trials"));
}

// Purchasing is the web server's: these commands ask its API as the
// signed-in person (../../../../tauri/src/purchasing.rs). Stripe's pages open
// in the system browser, so a checkout and the portal answer no `url`.

export function accountsProductsList() {
  return invoke("accounts_products_list");
}

/** @param {string} id */
export function accountsProductsGet(id) {
  return invoke("accounts_products_get", { id });
}

/** @param {string} priceId */
export function accountsCheckoutsCreate(priceId) {
  return invoke("accounts_checkouts_create", { priceId });
}

/** @param {string} id */
export function accountsCheckoutsGet(id) {
  return invoke("accounts_checkouts_get", { id });
}

export function accountsPurchasesList() {
  return invoke("accounts_purchases_list");
}

/** @param {string} id */
export function accountsPurchasesGet(id) {
  return invoke("accounts_purchases_get", { id });
}

export function accountsSubscriptionsList() {
  return invoke("accounts_subscriptions_list");
}

/** @param {string} id */
export function accountsSubscriptionsGet(id) {
  return invoke("accounts_subscriptions_get", { id });
}

/**
 * @param {string} id
 * @param {object} change
 */
export function accountsSubscriptionsUpdate(id, change) {
  return invoke("accounts_subscriptions_update", { id, change });
}

/** @param {string} id */
export function accountsSubscriptionsPortalCreate(id) {
  return invoke("accounts_subscriptions_portal_create", { id });
}

// A local user: someone using the app on this device without signing
// up, see docs/design/accounts-local-and-trial-users.md.

/**
 * Whether the accounts page offers Skip.
 * @returns {Promise<boolean>}
 */
export function localUsersCanCreate() {
  return invoke("local_users_can_create");
}

/** Skip: makes the device's local user and opens the app as them. */
export function localUsersCreate() {
  return invoke("local_users_create");
}

/** Moves what the local user made into the account whose sign-in offered it. */
export function localUsersMove() {
  return invoke("local_users_move");
}

/** Adds the account whose sign-in offered a move beside the local user. */
export function localUsersKeepSeparate() {
  return invoke("local_users_keep_separate");
}

export function postsList() {
  return invoke("posts_list");
}

/** @param {string} id */
export function postsGet(id) {
  return invoke("posts_get", { id });
}

/**
 * @param {string} body
 * @param {string[]} communityIds the communities the post is in
 */
export function postsCreate(body, communityIds) {
  return invoke("posts_create", { body, communityIds });
}

/**
 * @param {string} id
 * @param {string} body
 * @param {string[]} communityIds the communities the post is in, exactly
 */
export function postsUpdate(id, body, communityIds) {
  return invoke("posts_update", { id, body, communityIds });
}

/**
 * A page of the files the user sees on this device whose names contain
 * `name` in any case, of one of `content_types` (any for none), newest
 * first: `{total, files}`, each file
 * `{file, creator, communities, shown_as, state, error}`.
 * @param {{name?: string, content_types?: string[], offset?: number, limit: number}} filter at most 500 at once
 */
export function filesList(filter) {
  return invoke("files_list", { filter });
}

/** How long a list's name filter waits for a pause in typing: each a local read. */
export const filesListWaitMs = 50;

/**
 * The files the user sees on this device with any of these checksums,
 * up to 500 at once.
 * @param {string[]} sha256s
 */
export function filesDuplicatesList(sha256s) {
  return invoke("files_duplicates_list", { sha256s });
}

/** @param {string} id */
export function filesGet(id) {
  return invoke("files_get", { id });
}

/**
 * @param {string} id
 * @param {string} name
 * @param {string[]} communityIds its communities among the user's, exactly
 */
export function filesUpdate(id, name, communityIds) {
  return invoke("files_update", { id, name, communityIds });
}

/** @param {string} id */
export function filesDelete(id) {
  return invoke("files_delete", { id });
}

/** How many files a page creates at once: one, each a local write. */
export const filesCreateAtOnce = 1;

/**
 * Creates the file the person picked: kept on this device at once, and
 * sent to the server by the next sync; answers the file. The bytes are
 * the command's body, the name, the type and the communities its headers.
 * @param {File} file
 * @param {string} name
 * @param {string[]} communityIds the communities the file is in, one at least
 * @param {(fraction: number) => void} [_onProgress] nothing to wait for here
 * @param {string} [_sha256] worked out again in Rust, from the bytes it keeps
 */
export async function filesCreate(file, name, communityIds, _onProgress, _sha256) {
  const bytes = new Uint8Array(await file.arrayBuffer());
  const headers = {
    "x-file-name": encodeURIComponent(name),
    "x-file-content-type": file.type,
    "x-file-community-ids": communityIds.join(","),
  };
  return invoke("files_create", bytes, { headers });
}

/**
 * The copies this device keeps of files the server holds too.
 * @returns {Promise<{count: number, bytes: number} | null>}
 */
export function filesCacheGet() {
  return invoke("files_cache_get");
}

/** Removes those copies; each is fetched again when next shown. */
export function filesCacheDelete() {
  return invoke("files_cache_delete");
}

/** Whether this device copies an image to the clipboard. */
export function filesCopySupported() {
  return invoke("files_copy_supported");
}

/**
 * Copies the image to the clipboard.
 * @param {string} id
 */
export function filesCopy(id) {
  return invoke("files_copy", { id });
}

/** Whether this device saves a copy of a file outside the app. */
export function filesDownloadSupported() {
  return invoke("files_download_supported");
}

/**
 * Saves a copy of the file in the Downloads folder.
 * @param {string} id
 * @returns {Promise<string | null>} where it was saved
 */
export function filesDownload(id) {
  return invoke("files_download", { id });
}

/**
 * Where a page reads the file's bytes from: the app's `files` protocol,
 * which serves the copy on this device.
 * @param {string} id
 */
export function filesContentUrl(id) {
  return convertFileSrc(id, "files");
}

/** Every community, each with the user's membership when they are in it. */
export function communitiesList() {
  return invoke("communities_list");
}

/** One community, `{community, membership}`. @param {string} id */
export function communitiesGet(id) {
  return invoke("communities_get", { id });
}

/** @param {string} name */
export function communitiesCreate(name) {
  return invoke("communities_create", { name });
}

/**
 * @param {string} id
 * @param {string} name
 */
export function communitiesUpdate(id, name) {
  return invoke("communities_update", { id, name });
}

/** @param {string} id */
export function communitiesDelete(id) {
  return invoke("communities_delete", { id });
}

/** @param {string} communityId */
export function membershipsCreate(communityId) {
  return invoke("memberships_create", { communityId });
}

/** @param {string} communityId */
export function membershipsDelete(communityId) {
  return invoke("memberships_delete", { communityId });
}

/** @param {string} id */
export function postsDelete(id) {
  return invoke("posts_delete", { id });
}

/** One sync now: the report, or the error as the message. */
export function syncRun() {
  return invoke("sync_run");
}

/**
 * Works offline, or online again, which syncs at once.
 * @param {boolean} offline
 */
export function syncSetOffline(offline) {
  return invoke("sync_set_offline", { offline });
}

/** Whether working offline, what waits, what conflicted, where the pull is. */
export function syncStatus() {
  return invoke("sync_status");
}

/**
 * Calls back after every sync with `{report, error}`.
 * @param {(outcome: unknown) => void} callback
 * @returns {() => void} stops listening
 */
export function onSynced(callback) {
  const stopping = listen("synced", (event) => callback(event.payload));
  return () => {
    stopping.then((stop) => stop());
  };
}

export function conflictsList() {
  return invoke("conflicts_list");
}

export function pendingChangesList() {
  return invoke("pending_changes_list");
}

/** @param {string} id */
export function conflictsReapply(id) {
  return invoke("conflicts_reapply", { id });
}

/**
 * Sends a refused change again as it was.
 * @param {string} id
 */
export function conflictsTryAgain(id) {
  return invoke("conflicts_try_again", { id });
}

/** @param {string} id */
export function conflictsDelete(id) {
  return invoke("conflicts_delete", { id });
}

// The quick picker, the desktop app's second window (../../quick-picker/).

/**
 * A page of the signed-in person's images the clipboard takes whose
 * names contain `name` in any case, newest first: `{total, files}`, as
 * `filesList` answers.
 * @param {{name?: string, offset?: number, limit: number}} filter at most 500 at once
 */
export function quickPickerFilesList(filter) {
  return invoke("quick_picker_files_list", { filter });
}

/**
 * Copies the file's image, closes the picker and pastes where the person was typing.
 * @param {string} id
 */
export function quickPickerPaste(id) {
  return invoke("quick_picker_paste", { id });
}

export function quickPickerClose() {
  return invoke("quick_picker_close");
}

/** Closes the picker and brings the app's window to the front, where a sign-in starts. */
export function quickPickerOpenApp() {
  return invoke("quick_picker_open_app");
}

/**
 * Asks macOS for the Accessibility access pasting needs.
 * @returns {Promise<boolean>} whether it is granted now
 */
export function quickPickerAccessibilityRequest() {
  return invoke("quick_picker_accessibility_request");
}

/**
 * Calls back each time the picker opens, with `{accessibility_trusted}`.
 * @param {(opened: {accessibility_trusted: boolean}) => void} callback
 * @returns {() => void} stops listening
 */
export function onQuickPickerOpened(callback) {
  const stopping = listen("quick-picker-opened", (event) => callback(event.payload));
  return () => {
    stopping.then((stop) => stop());
  };
}

/**
 * Opens a public page, `terms` or `privacy`, in the system browser, since
 * the app never shows one (../../../../tauri/src/public_pages.rs).
 * @param {"terms" | "privacy"} page
 */
export function publicPagesOpen(page) {
  return invoke("public_pages_open", { page });
}

/** Mails the signed-in person the code that confirms deleting their account. */
export function accountsDeletionsCodeCreate() {
  return invoke("accounts_deletions_code_create");
}

/**
 * Deletes the account with the mailed code, then removes it from this
 * device (../../../../tauri/src/deletions.rs).
 * @param {string | null} code
 */
export function accountsDeletionsCreate(code) {
  return invoke("accounts_deletions_create", { code: code ?? "" });
}

// A phone's store (../../../../tauri/src/store_transactions.rs): its
// sheet, then the server records the proof, then the store's
// transaction is finished.

/**
 * The store this app sells through: `app_store`, `play_store`, or `stripe`
 * on the desktop; `none` on an account of a server other than the app's own.
 */
export function accountsStoreGet() {
  return invoke("accounts_store_get");
}

/**
 * The store's sheet for the price: `{purchase}` or `{subscription}` once
 * the server has it, else `{cancelled: true}` or `{pending: true}`.
 * @param {string} priceId
 */
export function accountsStoreTransactionsCreate(priceId) {
  return invoke("accounts_store_transactions_create", { priceId });
}

/** Restore purchases: `{recorded, refused}`. */
export function accountsStoreTransactionsRestore() {
  return invoke("accounts_store_transactions_restore");
}

/** The store's own page of subscriptions. */
export function accountsSubscriptionsManageOpen() {
  return invoke("accounts_subscriptions_manage_open");
}
