// The server the pages are on and what it offers, `currentServer.value`,
// reactive, as `currentUser` is: asked once, `currentServerLoad()`, and
// asked again when the account changes, since an account is one server's,
// and when the app fetched the server's settings anew. See "Server
// settings" in docs/design/multi-server.md: the settings only decide what
// the pages offer; the server refuses what is off all the same.
import { onServersCurrentChanged, serversCurrentGet } from "./core/index.js";

/**
 * @typedef {{features: {accounts_sign_up: boolean, accounts_sign_in_code: boolean}}} ServerSettings
 * @typedef {{display_name: string, host: string, default: boolean, settings: ServerSettings}} Server
 */

/** The server the pages are on; null until asked. */
export const currentServer = $state({ value: /** @type {Server | null} */ (null) });

/** @type {Promise<Server> | null} */
let current = null;
let listening = false;

/** @returns {Promise<Server>} */
export function currentServerLoad() {
  if (!listening) {
    listening = true;
    onServersCurrentChanged(() => currentServerReload());
  }
  current ??= serversCurrentGet().then(
    (server) => {
      currentServer.value = server;
      return server;
    },
    (problem) => {
      current = null;
      throw problem;
    },
  );
  return current;
}

/**
 * Asks again, after the account or the server's settings changed.
 * @returns {Promise<Server>}
 */
export function currentServerReload() {
  current = null;
  return currentServerLoad();
}

/**
 * Asks again once the account changed, since another account may be on
 * another server; nothing when no page asked yet.
 */
export function currentServerForget() {
  current = null;
  if (currentServer.value) currentServerLoad().catch(() => {});
}
