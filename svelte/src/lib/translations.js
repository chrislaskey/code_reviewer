// The pages' text in other languages. A page writes its English with
// ttag's `t` and `ngettext`, and shows a language's translation once
// `translationsUse` has loaded it, English otherwise. The translations are
// the gettext files in ../../translations, see the README's "Translations".
import { addLocale, useLocale } from "ttag";

const CATALOGS = import.meta.glob("../../translations/*.po", { import: "default" });

/**
 * Shows the pages in `locale` from here on, loading its translations first;
 * "en", or a language with no translations file, shows the English.
 * @param {string} locale
 */
export async function translationsUse(locale) {
  const load = Object.entries(CATALOGS).find(([path]) => localeOfPath(path) === locale)?.[1];
  if (!load) {
    useLocale("en");
    return;
  }
  const catalog = /** @type {import("ttag").LocaleData} */ (await load());
  addLocale(locale, catalog);
  useLocale(locale);
}

/** @param {string} path */
function localeOfPath(path) {
  return path.slice(path.lastIndexOf("/") + 1, -".po".length);
}
