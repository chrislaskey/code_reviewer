// The pages' text in other languages. A page writes its English with
// ttag's `t` and `ngettext`, and shows a language's translation once
// `translationsUse` has loaded it, English otherwise. The translations are
// the gettext files in ../../translations, see the README's "Translations".
//
// Which language: the person's choice, their `locale` preference
// (app_core::accounts::preferences), else the device's. The choice is
// kept in localStorage under `app:locale` too, so the pages start in it
// before anyone is asked; when the current user comes with another,
// `translationsFollow` keeps that and reloads the page in it once.
import { addLocale, useLocale } from "ttag";

const CATALOGS = import.meta.glob("../../translations/*.po", { import: "default" });
const DEFAULT_LOCALE = "en";
const CHOSEN_KEY = "app:locale";

/** Every language the pages are in, English first: Rust's `app::locales::SUPPORTED`. */
export const TRANSLATIONS_LOCALES = [DEFAULT_LOCALE, ...Object.keys(CATALOGS).map(localeOfPath).sort()];

/** The language the pages show now. */
let shown = DEFAULT_LOCALE;

/**
 * Shows the pages in `locale` from here on, loading its translations first;
 * "en", or a language with no translations file, shows the English.
 * @param {string} locale
 */
export async function translationsUse(locale) {
  const load = Object.entries(CATALOGS).find(([path]) => localeOfPath(path) === locale)?.[1];
  shown = load ? locale : DEFAULT_LOCALE;
  document.documentElement.lang = shown;
  if (!load) {
    useLocale(DEFAULT_LOCALE);
    return;
  }
  const catalog = /** @type {import("ttag").LocaleData} */ (await load());
  addLocale(locale, catalog);
  useLocale(locale);
}

/** The language the pages show now, as `translationsUse` set it. */
export function translationsShown() {
  return shown;
}

/**
 * The language to format dates and numbers in: the one shown, with the
 * device's region when the device is in that language, `es-MX` for `es`.
 */
export function translationsFormatLocale() {
  const requested = navigator.languages?.length ? navigator.languages : [navigator.language];
  return requested.find((tag) => tag.toLowerCase().split("-")[0] === shown) ?? shown;
}

/** Before the pages mount: the language last chosen on this device, else the device's. */
export function translationsStart() {
  return translationsUse(translationsLocaleShown(localStorage.getItem(CHOSEN_KEY)));
}

/**
 * The person's choice as the application has it, null to follow the
 * device: kept for the next start, and the page reloaded when that
 * shows another language than the one shown now.
 * @param {string | null} chosen
 */
export function translationsFollow(chosen) {
  if (chosen) localStorage.setItem(CHOSEN_KEY, chosen);
  else localStorage.removeItem(CHOSEN_KEY);
  if (translationsLocaleShown(chosen) !== shown) window.location.reload();
}

/**
 * The language the pages show for a choice: it, when they are in it,
 * else the device's.
 * @param {string | null} chosen
 */
export function translationsLocaleShown(chosen) {
  if (chosen && TRANSLATIONS_LOCALES.includes(chosen)) return chosen;
  return localeOfDevice();
}

/** The device's first language the pages are in, its language without the region next. */
function localeOfDevice() {
  const requested = navigator.languages?.length ? navigator.languages : [navigator.language];
  for (const tag of requested) {
    const locale = tag.toLowerCase();
    if (TRANSLATIONS_LOCALES.includes(locale)) return locale;
    const language = locale.split("-")[0];
    if (TRANSLATIONS_LOCALES.includes(language)) return language;
  }
  return DEFAULT_LOCALE;
}

/** @param {string} path */
function localeOfPath(path) {
  return path.slice(path.lastIndexOf("/") + 1, -".po".length);
}
