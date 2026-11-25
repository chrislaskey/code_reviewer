// Flash messages: short notices above the page, such as "Switched to
// Alice" or an error, shown by the layout (FlashMessages.svelte). Several
// may show at once, newest last. Each stays until it is closed or the
// next navigation starts. To show one on the page a navigation opens,
// call flashMessageShow after `await navigateToUrl(...)`. The text is
// translated by the caller when the message is shown, and stays in that
// language. Icons are heroicon `hero-*` names; each tone has its own.

/** @typedef {"info" | "success" | "warning" | "error"} FlashMessageTone */
/** @typedef {{id: number, text: string, tone: FlashMessageTone, icon: string | null}} FlashMessage */

/** The most messages shown at once; a new one drops the oldest. */
const MAX_FLASH_MESSAGES = 5;

/** @type {Record<FlashMessageTone, string>} */
const TONE_ICONS = {
  info: "hero-information-circle",
  success: "hero-check-circle",
  warning: "hero-exclamation-triangle",
  error: "hero-x-circle",
};

/** @type {{value: FlashMessage[]}} */
export const flashMessages = $state({ value: [] });

let nextId = 1;

/**
 * Shows a message above the page. The same text in the same tone shows once.
 * @param {string} text already translated
 * @param {{tone?: FlashMessageTone, icon?: string | null}} [options] `icon`
 *   replaces the tone's own; null shows none
 * @returns {number} the message's id, for flashMessageClose
 */
export function flashMessageShow(text, { tone = "info", icon } = {}) {
  const shown = flashMessages.value.find((message) => message.text === text && message.tone === tone);
  if (shown) return shown.id;
  const message = { id: nextId++, text, tone, icon: icon === undefined ? TONE_ICONS[tone] : icon };
  flashMessages.value = [...flashMessages.value, message].slice(-MAX_FLASH_MESSAGES);
  return message.id;
}

/** @param {number} id */
export function flashMessageClose(id) {
  flashMessages.value = flashMessages.value.filter((message) => message.id !== id);
}

/** Closes every message; the router calls it as a navigation starts. */
export function flashMessagesClear() {
  flashMessages.value = [];
}
