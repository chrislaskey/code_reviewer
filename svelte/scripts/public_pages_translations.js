// The public pages' translations, see docs/design/public-pages.md. A page
// (src/public) marks each piece of its English `<t>…</t>`, a paragraph
// with its links, a heading, a link's text, its title and description;
// that English, its whitespace as one space, is the message. Their own
// catalog is translations/public, `default.pot` and a `<locale>.po` per
// language, apart from the app's pages'. The build writes each page in
// English at its path, `/about`, and in each language under it,
// `/es/about`, where its links to other public pages stay in the
// language, and its footer links the page in every other language.
import { readFileSync, readdirSync } from "node:fs";
import { resolve } from "node:path";
import { po } from "gettext-parser";

export const PUBLIC_TRANSLATIONS = resolve(import.meta.dirname, "../translations/public");
export const PUBLIC_PAGES = resolve(import.meta.dirname, "../src/public");
export const PUBLIC_DEFAULT_LOCALE = "en";
const MESSAGE = /<t>([\s\S]*?)<\/t>/g;
const LANGUAGES_MARKER = "<!-- public-languages -->";

/**
 * The message for a piece of English as a page writes it.
 * @param {string} english
 */
export function publicMessageOf(english) {
  return english.replace(/\s+/g, " ").trim();
}

/**
 * The messages a page's HTML marks, with the line each is on.
 * @param {string} html
 * @returns {{message: string, line: number}[]}
 */
export function publicMessages(html) {
  return [...html.matchAll(MESSAGE)].map((match) => ({
    message: publicMessageOf(match[1]),
    line: html.slice(0, match.index).split("\n").length,
  }));
}

/**
 * Every message of the public pages and their partials, with where each
 * is, as `src/public/about.html:14`.
 * @returns {Map<string, string[]>}
 */
export function publicPagesMessages() {
  const messages = new Map();
  for (const [file, html] of publicPagesFiles()) {
    for (const { message, line } of publicMessages(html)) {
      messages.set(message, [...(messages.get(message) ?? []), `src/public/${file}:${line}`]);
    }
  }
  return messages;
}

/**
 * The English in the public pages and their partials written outside
 * `<t>…</t>`: text between tags, and the description and the attributes
 * a person reads or hears. A comment that starts with "untranslated"
 * exempts the element after it, as in the app's pages.
 * @returns {string[]} as `src/public/about.html:12: "text"`
 */
export function publicPagesUntranslated() {
  const problems = [];
  for (const [file, html] of publicPagesFiles()) {
    // Each part taken out keeps its lines, so a line is still its own
    const blank = (part) => part.replace(/[^\n]/g, " ");
    const left = html
      .replace(/<!--\s*untranslated[\s\S]*?-->\s*<(\w+)[^>]*>[\s\S]*?<\/\1>/g, blank)
      .replace(/<!--[\s\S]*?-->/g, blank)
      .replace(/<(script|style)[^>]*>[\s\S]*?<\/\1>/g, blank)
      .replace(/<!doctype[^>]*>/gi, blank)
      .replace(MESSAGE, blank);
    const found = (index, text) => {
      const line = left.slice(0, index).split("\n").length;
      problems.push(`src/public/${file}:${line}: ${JSON.stringify(publicMessageOf(text))}`);
    };
    for (const match of left.matchAll(/<[^>]*>/g)) {
      for (const attribute of match[0].matchAll(/\b(alt|aria-label|placeholder|title)="([^"]*)"/g)) {
        if (/\p{L}/u.test(attribute[2])) found(match.index, `${attribute[1]}="${attribute[2]}"`);
      }
      const description = /<meta name="description" content="([^"]*)"/.exec(match[0]);
      if (description && /\p{L}/u.test(description[1])) found(match.index, `description="${description[1]}"`);
    }
    for (const match of left.replace(/<[^>]*>/g, blank).matchAll(/[^\n]*\p{L}[^\n]*/gu)) {
      found(match.index, match[0]);
    }
  }
  return problems;
}

/**
 * The public pages' and partials' HTML, by file from src/public.
 * @returns {[string, string][]}
 */
function publicPagesFiles() {
  return [
    ...readdirSync(PUBLIC_PAGES).filter((file) => file.endsWith(".html")),
    ...readdirSync(resolve(PUBLIC_PAGES, "partials")).map((file) => `partials/${file}`),
  ]
    .sort()
    .map((file) => [file, readFileSync(resolve(PUBLIC_PAGES, file), "utf8")]);
}

/**
 * Every language's translations, by message, without the fuzzy ones and
 * those not translated, which show the English.
 * @returns {Map<string, Map<string, string>>}
 */
export function publicCatalogs() {
  const catalogs = new Map();
  for (const file of readdirSync(PUBLIC_TRANSLATIONS).filter((file) => file.endsWith(".po")).sort()) {
    const catalog = po.parse(readFileSync(resolve(PUBLIC_TRANSLATIONS, file)));
    const translations = new Map();
    for (const message of Object.values(catalog.translations[""] ?? {})) {
      const isFuzzy = message.comments?.flag?.split(",").some((flag) => flag.trim() === "fuzzy");
      if (message.msgid && message.msgstr[0] && !isFuzzy) translations.set(message.msgid, message.msgstr[0]);
    }
    catalogs.set(file.slice(0, -".po".length), translations);
  }
  return catalogs;
}

/**
 * The path a public page answers at in a language: `/about`, `/es/about`.
 * @param {string} locale
 * @param {string} name the page's file name, `index` for the home page
 */
export function publicPagePath(locale, name) {
  const prefix = locale === PUBLIC_DEFAULT_LOCALE ? "" : `/${locale}`;
  if (name === "index") return prefix || "/";
  return `${prefix}/${name}`;
}

/**
 * A page's HTML in a language: each message translated, English where it
 * has none, with `<html lang>`, its links to the public pages in that
 * language, and the footer's links to the page in the others.
 * @param {string} html with its messages marked
 * @param {{locale: string, name: string, pageNames: string[], catalogs: Map<string, Map<string, string>>}} page
 */
export function publicPageTranslated(html, { locale, name, pageNames, catalogs }) {
  const translations = catalogs.get(locale);
  let translated = html.replace(MESSAGE, (_, english) => translations?.get(publicMessageOf(english)) ?? english);
  if (locale !== PUBLIC_DEFAULT_LOCALE) {
    translated = translated
      .replace(/<html lang="[^"]*"/, `<html lang="${locale}"`)
      .replace(/href="\/([\w-]*)"/g, (link, target) => {
        const page = target === "" ? "index" : target;
        return pageNames.includes(page) ? `href="${publicPagePath(locale, page)}"` : link;
      });
  }
  const locales = [PUBLIC_DEFAULT_LOCALE, ...catalogs.keys()].filter((other) => other !== locale);
  const languageLinks = locales
    .map((other) => {
      const languageName = new Intl.DisplayNames([other], { type: "language" }).of(other) ?? other;
      const shown = languageName.charAt(0).toLocaleUpperCase(other) + languageName.slice(1);
      return `<a href="${publicPagePath(other, name)}" lang="${other}" hreflang="${other}" class="link">${shown}</a>`;
    })
    .join("\n  ");
  return translated.replace(LANGUAGES_MARKER, () => languageLinks);
}
