// The user signed in, as the pages and the nav show them, `currentUser.value`,
// reactive, so every component reading it shows the change at once: asked
// of the application once, `currentUserLoad()`, and kept until a sign-out
// or a sign-in says otherwise. Null when nobody is signed in, which is what
// sends a visitor to the sign-in page (router.js); an error only when the
// application could not tell, which the page shows.
//
// Whether someone is signed in is also kept in localStorage, under
// `app:signed-in`, for the public website's header (src/public/partials),
// which has no way to ask: it shows "Open the app" in place of Sign in
// and Sign up. Only a hint, so a stale one costs a click.
import { currentUserGet } from "./core/index.js";
import { currentServerForget } from "./current_server.svelte.js";

/**
 * The user with what only they are shown: their account's email, on the
 * web whether they are staff, and what kind of user they are: `full`,
 * signed up; `local`, on this device only, in the app; `trial`, on the
 * web until `trial_expires_at` unless they sign up. See
 * docs/design/accounts-local-and-trial-users.md.
 * @typedef {{grant: string, product_id: string, product_name: string, price_id: string, subscription_id: string, store: "stripe" | "app_store" | "play_store", status: string, current_period_end: string | null, cancel_at_period_end: boolean}} Plan
 * @typedef {{id: string, name: string, email: string, kind: "full" | "local" | "trial", trial_expires_at?: string | null, is_staff?: boolean, avatar_url?: string | null, plan?: Plan | null, payment_failed_subscription_id?: string | null}} User
 */

/** The user signed in; null until asked and while nobody is. */
export const currentUser = $state({ value: /** @type {User | null} */ (null) });

/** @type {Promise<User | null> | null} */
let current = null;

const SIGNED_IN_KEY = "app:signed-in";

/** @param {boolean} signedIn */
function rememberSignedIn(signedIn) {
  if (signedIn) localStorage.setItem(SIGNED_IN_KEY, "true");
  else localStorage.removeItem(SIGNED_IN_KEY);
}

/** @returns {Promise<User | null>} */
export function currentUserLoad() {
  current ??= currentUserGet().then(
    (user) => {
      if (!user) current = null;
      currentUser.value = user;
      rememberSignedIn(Boolean(user));
      return user;
    },
    (problem) => {
      current = null;
      currentUser.value = null;
      throw problem;
    },
  );
  return current;
}

/**
 * Asks again, after something that changes the user as their pages show
 * them, such as buying a plan.
 * @returns {Promise<User | null>}
 */
export function currentUserReload() {
  current = null;
  return currentUserLoad();
}

export function currentUserForget() {
  current = null;
  currentUser.value = null;
  rememberSignedIn(false);
  currentServerForget();
}
