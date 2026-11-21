// What a page shows, asked of app_core when the page opens: its value,
// or the message of the error it met, and whether it is still loading.
// With `reloadOnSync`, asked again after every sync, so the page shows
// what another device changed. Call it in a component's <script>, as
// it listens for syncs for as long as the component is shown.
import { onSynced } from "./core/index.js";
import { errorMessageOf } from "./error_message.js";

/**
 * @template T
 * @typedef {{readonly value: T | undefined, readonly error: string, readonly loading: boolean, reload: () => Promise<void>}} Loaded
 */

/**
 * @template T
 * @param {() => Promise<T>} load
 * @param {{reloadOnSync?: boolean}} [options]
 * @returns {Loaded<T>}
 */
export function loaded(load, { reloadOnSync = false } = {}) {
  let value = $state(/** @type {T | undefined} */ (undefined));
  let error = $state("");
  let loading = $state(true);

  async function reload() {
    try {
      value = await load();
      error = "";
    } catch (problem) {
      error = errorMessageOf(problem);
    } finally {
      loading = false;
    }
  }

  if (reloadOnSync) $effect(() => onSynced(reload));
  reload();

  return {
    get value() {
      return value;
    },
    get error() {
      return error;
    },
    get loading() {
      return loading;
    },
    reload,
  };
}
