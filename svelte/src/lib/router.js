// The router: the page at each path, swapped in place as the address
// changes, so switching pages never shows a blank window. The paths are
// routes.js's, the one module that spells them out. A page is loaded the
// first time it is visited, so each page's code stays a chunk of its own.
// Before a page that needs someone signed in, the router asks who is,
// and sends a visitor to the sign-in page instead when nobody is, or in
// the app to the accounts on the device, where sign-in is reached from;
// the app never shows the sign-in pages themselves. When it cannot tell,
// or a page's code does not load, the page says so above it rather than
// going nowhere (page_load_error.svelte.js). The template's own pages are
// listed here, the app's in pages.js.
import { Navigation, createRouter } from "sv-router";
import { t } from "ttag";
import AppLayout from "./layouts/AppLayout.svelte";
import { APP_PAGES } from "./pages.js";
import { hasDeviceAccounts } from "./core/index.js";
import { currentUserLoad } from "./current_user.svelte.js";
import { errorMessageOf } from "./error_message.js";
import { pageLoadErrorClear, pageLoadErrorShow } from "./page_load_error.svelte.js";
import {
  ACCOUNTS_INDEX_PATH,
  ACCOUNTS_CURRENT_PASSWORD_EDIT_PATH,
  ACCOUNTS_DELETIONS_NEW_PATH,
  ACCOUNTS_PRODUCTS_INDEX_PATH,
  ACCOUNTS_PRODUCTS_SHOW_PATH,
  ACCOUNTS_CHECKOUTS_SHOW_PATH,
  ACCOUNTS_BACK_TO_APP_PATH,
  ACCOUNTS_PURCHASES_INDEX_PATH,
  ACCOUNTS_PURCHASES_SHOW_PATH,
  ACCOUNTS_SUBSCRIPTIONS_INDEX_PATH,
  ACCOUNTS_SUBSCRIPTIONS_SHOW_PATH,
  ACCOUNTS_CURRENT_SHOW_PATH,
  ACCOUNTS_SIGN_IN_PATH,
  ACCOUNTS_TRIAL_PATH,
  COMMUNITIES_EDIT_PATH,
  COMMUNITIES_INDEX_PATH,
  COMMUNITIES_NEW_PATH,
  COMMUNITIES_SHOW_PATH,
  CONFLICTS_INDEX_PATH,
  HOME_PATH,
  OPEN_PATHS,
  PUBLIC_HOME_PATH,
  accountsIndexUrl,
  accountsSignInUrl,
  homeUrl,
} from "./routes.js";

const { navigate, route } = createRouter({
  layout: AppLayout,
  hooks: {
    async beforeLoad({ pathname }) {
      pageLoadErrorClear();
      // `/` is the public website's on the web; the app's home is its own path
      if (pathname === PUBLIC_HOME_PATH) throw navigateToUrl(homeUrl());
      // In the app a sign-in happens in the browser, from the accounts page
      const webOnly = [ACCOUNTS_SIGN_IN_PATH, ACCOUNTS_TRIAL_PATH];
      if (hasDeviceAccounts && webOnly.includes(pathname)) {
        throw navigateToUrl(accountsIndexUrl());
      }
      if (OPEN_PATHS.includes(pathname)) return;
      let user;
      try {
        user = await currentUserLoad();
      } catch (problem) {
        const reason = errorMessageOf(problem);
        pageLoadErrorShow(t`Could not check who is signed in: ${reason}`);
        return;
      }
      if (!user) throw navigateToUrl(hasDeviceAccounts ? accountsIndexUrl() : accountsSignInUrl());
    },
    onError(problem) {
      // A redirect from beforeLoad arrives here too
      if (problem instanceof Navigation || problem instanceof Promise) return;
      const reason = errorMessageOf(problem);
      pageLoadErrorShow(t`This page could not load: ${reason}`);
    },
  },
  ...APP_PAGES,
  [ACCOUNTS_INDEX_PATH]: () => import("../accounts/AccountsIndexPage.svelte"),
  [ACCOUNTS_SIGN_IN_PATH]: () => import("../accounts/AccountsSignInPage.svelte"),
  [ACCOUNTS_TRIAL_PATH]: () => import("../accounts/AccountsTrialPage.svelte"),
  [COMMUNITIES_INDEX_PATH]: () => import("../communities/CommunitiesIndexPage.svelte"),
  [COMMUNITIES_NEW_PATH]: () => import("../communities/CommunitiesNewPage.svelte"),
  [COMMUNITIES_SHOW_PATH]: () => import("../communities/CommunitiesShowPage.svelte"),
  [COMMUNITIES_EDIT_PATH]: () => import("../communities/CommunitiesEditPage.svelte"),
  [CONFLICTS_INDEX_PATH]: () => import("../conflicts/ConflictsIndexPage.svelte"),
  [ACCOUNTS_CURRENT_SHOW_PATH]: () => import("../accounts/current/AccountsCurrentShowPage.svelte"),
  [ACCOUNTS_CURRENT_PASSWORD_EDIT_PATH]: () => import("../accounts/current/password/AccountsCurrentPasswordEditPage.svelte"),
  [ACCOUNTS_DELETIONS_NEW_PATH]: () => import("../accounts/deletions/AccountsDeletionsNewPage.svelte"),
  [ACCOUNTS_PRODUCTS_INDEX_PATH]: () => import("../accounts/products/AccountsProductsIndexPage.svelte"),
  [ACCOUNTS_PRODUCTS_SHOW_PATH]: () => import("../accounts/products/AccountsProductsShowPage.svelte"),
  [ACCOUNTS_CHECKOUTS_SHOW_PATH]: () => import("../accounts/checkouts/AccountsCheckoutsShowPage.svelte"),
  [ACCOUNTS_BACK_TO_APP_PATH]: () => import("../accounts/AccountsBackToAppPage.svelte"),
  [ACCOUNTS_PURCHASES_INDEX_PATH]: () => import("../accounts/purchases/AccountsPurchasesIndexPage.svelte"),
  [ACCOUNTS_PURCHASES_SHOW_PATH]: () => import("../accounts/purchases/AccountsPurchasesShowPage.svelte"),
  [ACCOUNTS_SUBSCRIPTIONS_INDEX_PATH]: () => import("../accounts/subscriptions/AccountsSubscriptionsIndexPage.svelte"),
  [ACCOUNTS_SUBSCRIPTIONS_SHOW_PATH]: () => import("../accounts/subscriptions/AccountsSubscriptionsShowPage.svelte"),
});

/**
 * The current page's path parameters, such as `id` in `/posts/:id`.
 * @returns {Record<string, string>}
 */
export function pathParams() {
  return /** @type {Record<string, string>} */ (route.params);
}

/** The current page's path, such as `/posts/1`; reactive. */
export function currentPathname() {
  return route.pathname;
}

// The router types its paths; ours come from routes.js, so any path goes.
const navigateToPath = /** @type {(pathname: string) => Promise<unknown>} */ (/** @type {unknown} */ (navigate));

/**
 * Shows the page at a URL from routes.js in place of the current one, and
 * adds it to the history as a link would.
 * @param {string} url
 */
export function navigateToUrl(url) {
  return navigateToPath(url);
}
