// The Rust catalog, core/translations: the English Rust shows a person
// itself, the emails, written in `t!(locale, "…")` (core/src/helpers/
// translations.rs), apart from the pages' two catalogs. Rust reads each
// language compiled, `<locale>.mo`, which `pnpm translations:update`
// writes from its `.po` and `translations:check` checks is current.
import { readFileSync, readdirSync } from "node:fs";
import { join, relative, resolve } from "node:path";
import { mo, po } from "gettext-parser";

export const RUST_TRANSLATIONS = resolve(import.meta.dirname, "../../core/translations");
const RUST_SOURCES = resolve(import.meta.dirname, "../../core/src");
const CALL = /\bt!\(\s*(?:"[^"]*"|[^,"]+),\s*"((?:[^"\\]|\\[\s\S])*)"/g;

/**
 * Every message of a `t!` in core/src, with where each is, as
 * `core/src/services/mailer/mails.rs:16`.
 * @returns {Map<string, string[]>}
 */
export function rustMessages() {
  const messages = new Map();
  for (const path of rustFiles(RUST_SOURCES)) {
    // Comments blanked, keeping their lines, so a doc's example is no message
    const source = readFileSync(path, "utf8").replace(/\/\/[^\n]*/g, "");
    for (const match of source.matchAll(CALL)) {
      const line = source.slice(0, match.index).split("\n").length;
      const message = rustStringOf(match[1]);
      const where = `${relative(resolve(RUST_SOURCES, "../.."), path)}:${line}`;
      messages.set(message, [...(messages.get(message) ?? []), where]);
    }
  }
  return messages;
}

/**
 * A `.po` compiled as Rust reads it, without the fuzzy messages and
 * those not translated, which show the English.
 * @param {string} poFile
 */
export function rustCompiled(poFile) {
  const catalog = po.parse(readFileSync(poFile));
  for (const messages of Object.values(catalog.translations)) {
    for (const [key, message] of Object.entries(messages)) {
      if (key === "") continue;
      const isFuzzy = message.comments?.flag?.split(",").some((flag) => flag.trim() === "fuzzy");
      if (isFuzzy || message.msgstr.some((translation) => translation === "")) delete messages[key];
      else delete message.comments;
    }
  }
  return mo.compile(catalog);
}

/** The `.po` files of the Rust catalog. */
export function rustPoFiles() {
  return readdirSync(RUST_TRANSLATIONS)
    .filter((file) => file.endsWith(".po"))
    .sort()
    .map((file) => join(RUST_TRANSLATIONS, file));
}

/**
 * A Rust string literal's text: its escapes, and a line ended by `\`
 * joined to the next without its leading spaces, as Rust reads it.
 * @param {string} literal between the quotes
 */
function rustStringOf(literal) {
  return literal.replace(/\\(\n\s*|n|t|r|"|'|\\|0)/g, (_, escape) => {
    if (escape.startsWith("\n")) return "";
    return { n: "\n", t: "\t", r: "\r", '"': '"', "'": "'", "\\": "\\", 0: "\0" }[escape];
  });
}

/** @param {string} folder */
function rustFiles(folder) {
  return readdirSync(folder, { withFileTypes: true })
    .flatMap((entry) => {
      const path = join(folder, entry.name);
      if (entry.isDirectory()) return rustFiles(path);
      return entry.name.endsWith(".rs") ? [path] : [];
    })
    .sort();
}
