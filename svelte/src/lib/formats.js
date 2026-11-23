// Times and numbers as a person reads them, in the language the pages
// show (translations.js): a time the server sent, `2026-11-23T12:33:06Z`,
// as `Nov 23, 2026, 12:33 PM` or `23 nov 2026, 12:33`, in the device's
// time zone.
import { translationsFormatLocale } from "./translations.js";

/**
 * A day and its time.
 * @param {string | number | Date} at
 */
export function dateTimeFormat(at) {
  return dateOf(at).toLocaleString(translationsFormatLocale(), { dateStyle: "medium", timeStyle: "short" });
}

/**
 * A day.
 * @param {string | number | Date} at
 */
export function dateFormat(at) {
  return dateOf(at).toLocaleDateString(translationsFormatLocale(), { dateStyle: "medium" });
}

/**
 * A number, its digits grouped: `5,000` or `5.000`.
 * @param {number} number
 * @param {Intl.NumberFormatOptions} [options]
 */
export function numberFormat(number, options) {
  return new Intl.NumberFormat(translationsFormatLocale(), options).format(number);
}

/**
 * The time, its fraction cut to milliseconds: the server sends
 * microseconds, which not every browser parses.
 * @param {string | number | Date} at
 */
function dateOf(at) {
  return new Date(typeof at === "string" ? at.replace(/(\.\d{3})\d+/, "$1") : at);
}
