// The community the person works in, as the pages and the nav show it,
// `currentCommunity.value`, reactive: asked of the application,
// `currentCommunityLoad()`, which answers null when they are in none, when
// the nav opens, after each sync, and after joining or leaving one.
import { currentCommunityGet } from "./core/index.js";

/** @typedef {{id: string, name: string}} Community */

/** The current community; null until asked and while there is none. */
export const currentCommunity = $state({ value: /** @type {Community | null} */ (null) });

/** Asks for the current community, and keeps it in `currentCommunity`. */
export async function currentCommunityLoad() {
  currentCommunity.value = await currentCommunityGet();
  return currentCommunity.value;
}
