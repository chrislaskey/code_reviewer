// The pages' calls into app_core (../../../../core). Inside the Tauri app
// they are the app's commands; in a browser they are the web server's JSON
// API. Both answer with the same data, serialized from the same Rust
// types, so a page never knows which it is on. Add a function to both
// files when app_core gains one.
import { isTauri } from "@tauri-apps/api/core";
import * as httpApi from "./http-api.js";
import * as tauriCommands from "./tauri-commands.js";
import { putFile } from "./uploads.js";

export { FormError } from "./form_error.js";
export { sha256Of as filesSha256 } from "./checksums.js";

const core = isTauri() ? tauriCommands : httpApi;

export const currentUserGet = core.currentUserGet;
export const currentUserUpdate = core.currentUserUpdate;
export const currentCommunityGet = core.currentCommunityGet;
export const currentCommunityUpdate = core.currentCommunityUpdate;
export const avatarsCreateUpload = core.avatarsCreateUpload;
export const avatarsUpdate = core.avatarsUpdate;
export const accountsSignUp = core.accountsSignUp;
export const accountsSignIn = core.accountsSignIn;
export const accountsCode = core.accountsCode;
export const accountsCodeResend = core.accountsCodeResend;
export const accountsSignInPassword = core.accountsSignInPassword;
export const accountsSignInNewPassword = core.accountsSignInNewPassword;
export const accountsSignInResetPassword = core.accountsSignInResetPassword;
export const accountsSecondFactors = core.accountsSecondFactors;
export const accountsPasswordUpdate = core.accountsPasswordUpdate;
export const accountsDeletionsCodeCreate = core.accountsDeletionsCodeCreate;
export const accountsDeletionsCreate = core.accountsDeletionsCreate;
export const accountsSignInStart = core.accountsSignInStart;
export const accountsSignInPending = core.accountsSignInPending;
export const accountsSignInFinish = core.accountsSignInFinish;
export const accountsSignInCancel = core.accountsSignInCancel;
export const accountsSignOut = core.accountsSignOut;
export const hasDeviceAccounts = core.hasDeviceAccounts;
export const deviceAccountsList = core.deviceAccountsList;
export const deviceAccountsSwitch = core.deviceAccountsSwitch;
export const deviceAccountsDeactivate = core.deviceAccountsDeactivate;
export const deviceAccountsRemove = core.deviceAccountsRemove;
export const accountsTrialGet = core.accountsTrialGet;
export const accountsTrialCreate = core.accountsTrialCreate;
export const accountsProductsList = core.accountsProductsList;
export const accountsProductsGet = core.accountsProductsGet;
export const accountsCheckoutsCreate = core.accountsCheckoutsCreate;
export const accountsCheckoutsGet = core.accountsCheckoutsGet;
export const accountsStoreGet = core.accountsStoreGet;
export const accountsStoreTransactionsCreate = core.accountsStoreTransactionsCreate;
export const accountsStoreTransactionsRestore = core.accountsStoreTransactionsRestore;
export const accountsSubscriptionsManageOpen = core.accountsSubscriptionsManageOpen;
export const accountsPurchasesList = core.accountsPurchasesList;
export const accountsPurchasesGet = core.accountsPurchasesGet;
export const accountsSubscriptionsList = core.accountsSubscriptionsList;
export const accountsSubscriptionsGet = core.accountsSubscriptionsGet;
export const accountsSubscriptionsUpdate = core.accountsSubscriptionsUpdate;
export const accountsSubscriptionsPortalCreate = core.accountsSubscriptionsPortalCreate;
export const localUsersCanCreate = core.localUsersCanCreate;
export const localUsersCreate = core.localUsersCreate;
export const localUsersMove = core.localUsersMove;
export const localUsersKeepSeparate = core.localUsersKeepSeparate;
export const postsList = core.postsList;
export const postsGet = core.postsGet;
export const postsCreate = core.postsCreate;
export const postsUpdate = core.postsUpdate;
export const postsDelete = core.postsDelete;
export const filesSearch = core.filesSearch;
export const filesSearchWaitMs = core.filesSearchWaitMs;
export const filesListDuplicates = core.filesListDuplicates;
export const filesGet = core.filesGet;
export const filesCreate = core.filesCreate;
export const filesCreateAtOnce = core.filesCreateAtOnce;
export const filesUpdate = core.filesUpdate;
export const filesDelete = core.filesDelete;
export const filesContentUrl = core.filesContentUrl;
export const filesCacheGet = core.filesCacheGet;
export const filesCacheDelete = core.filesCacheDelete;
export const filesCopySupported = core.filesCopySupported;
export const filesCopy = core.filesCopy;
export const filesDownloadSupported = core.filesDownloadSupported;
export const filesDownload = core.filesDownload;
export const communitiesList = core.communitiesList;
export const communitiesGet = core.communitiesGet;
export const communitiesCreate = core.communitiesCreate;
export const communitiesUpdate = core.communitiesUpdate;
export const communitiesDelete = core.communitiesDelete;
export const membershipsJoin = core.membershipsJoin;
export const membershipsLeave = core.membershipsLeave;
export const syncRun = core.syncRun;
export const syncStatus = core.syncStatus;
export const syncSetOffline = core.syncSetOffline;
export const onSynced = core.onSynced;
export const conflictsList = core.conflictsList;
export const conflictsReapply = core.conflictsReapply;
export const conflictsTryAgain = core.conflictsTryAgain;
export const conflictsLetGo = core.conflictsLetGo;
export const pendingChangesList = core.pendingChangesList;
export const quickPickerList = core.quickPickerList;
export const quickPickerPaste = core.quickPickerPaste;
export const quickPickerClose = core.quickPickerClose;
export const quickPickerOpenApp = core.quickPickerOpenApp;
export const quickPickerAccessibilityRequest = core.quickPickerAccessibilityRequest;
export const onQuickPickerOpened = core.onQuickPickerOpened;
export const publicPagesOpen = core.publicPagesOpen;

/**
 * Uploads the picture straight to object storage, then makes it the
 * user's picture; answers the user. Throws a FormError for a file the
 * server does not take.
 * @param {File} file
 * @param {(fraction: number) => void} [onProgress] from 0 to 1
 */
export async function avatarsUpload(file, onProgress) {
  const upload = await core.avatarsCreateUpload(file.type, file.size);
  await putFile(upload, file, onProgress);
  return core.avatarsUpdate(upload.key);
}
