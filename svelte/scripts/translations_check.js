// Checks the translations, `pnpm translations:check`: that the pages'
// markup writes its English through ttag (untranslated_text.js) and the
// public pages' marks theirs `<t>…</t>` (public_pages_translations.js);
// then, for each catalog, the app's pages' in translations/, the public
// pages' in translations/public/ and Rust's in ../core/translations/,
// with the messages extracted afresh, that its default.pot has them all
// and no others, and that every <locale>.po beside it translates each of
// them, none fuzzy; and for Rust's, that each <locale>.mo is its .po
// compiled, and each translation keeps the English's `{placeholders}`.
// Prints what is wrong and exits 1.
import { execFileSync } from "node:child_process";
import { existsSync, mkdtempSync, readFileSync, readdirSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, relative, resolve } from "node:path";
import { po } from "gettext-parser";
import { PUBLIC_TRANSLATIONS, publicPagesMessages, publicPagesUntranslated } from "./public_pages_translations.js";
import { RUST_TRANSLATIONS, rustCompiled, rustMessages, rustPoFiles } from "./rust_translations.js";
import { untranslatedText } from "./untranslated_text.js";

const SVELTE = resolve(import.meta.dirname, "..");
const TRANSLATIONS = resolve(SVELTE, "translations");
const ROOT = resolve(SVELTE, "..");
const COMMITTED_POT = "default.pot";
// The same sources as package.json's translations:extract
const IGNORED = "src/ops";
const EXTRACT_SOURCES = ["--ignore", `${IGNORED}/**`, "src"];

const untranslated = [
  ...untranslatedText(SVELTE, resolve(SVELTE, "src"), [IGNORED]),
  ...publicPagesUntranslated(),
];
const catalogs = [
  { folder: TRANSLATIONS, extracted: messagesExtract() },
  { folder: PUBLIC_TRANSLATIONS, extracted: publicMessagesExtracted() },
  { folder: RUST_TRANSLATIONS, extracted: messagesOf(rustMessages()) },
];
const problems = catalogs.flatMap(({ folder, extracted }) => {
  const shown = (file) => relative(ROOT, resolve(folder, file));
  return [
    ...messagesCompare(shown(COMMITTED_POT), messagesRead(resolve(folder, COMMITTED_POT)), extracted),
    ...localeFiles(folder).flatMap((file) =>
      localeProblems(shown(file), messagesRead(resolve(folder, file)), extracted),
    ),
  ];
});
problems.push(...rustProblems());

if (untranslated.length > 0) {
  for (const problem of untranslated) console.error(problem);
  console.error(
    "\nWrite these through ttag, `{t`...`}`, in a public page `<t>…</t>`, or put `<!-- untranslated: why -->` before one that stays as it is.\n",
  );
}
if (problems.length > 0) {
  for (const problem of problems) console.error(problem);
  console.error(
    "\nRun `pnpm translations:update`, then translate what is untranslated or fuzzy in each .po and remove its fuzzy flag.",
  );
}
if (untranslated.length > 0 || problems.length > 0) process.exit(1);
for (const { folder, extracted } of catalogs) {
  const locales = localeFiles(folder).map((file) => file.slice(0, -".po".length));
  console.log(
    `Translations, ${relative(ROOT, folder)}: ${extracted.size} messages, translated into: ${locales.join(", ") || "none yet"}.`,
  );
}

/** The public pages' messages, as messagesRead answers a file's. */
function publicMessagesExtracted() {
  return messagesOf(publicPagesMessages());
}

/**
 * Messages found in the sources, as messagesRead answers a file's.
 * @param {Map<string, string[]>} found
 */
function messagesOf(found) {
  return new Map([...found.keys()].map((message) => [message, { msgid: message, msgstr: [""] }]));
}

/** Rust's: a `.mo` not compiled from its `.po`, a placeholder a translation lost or made up. */
function rustProblems() {
  const problems = [];
  const placeholders = (text) => (text.match(/\{[a-z_]+\}/g) ?? []).sort().join(" ");
  for (const poFile of rustPoFiles()) {
    const moFile = poFile.replace(/\.po$/, ".mo");
    const compiled = rustCompiled(poFile);
    const committed = existsSync(moFile) ? readFileSync(moFile) : Buffer.alloc(0);
    if (!compiled.equals(committed)) problems.push(`${relative(ROOT, moFile)}: not compiled from its .po`);
    for (const [key, message] of messagesRead(poFile)) {
      const translated = message.msgstr.filter(Boolean);
      if (translated.some((translation) => placeholders(translation) !== placeholders(message.msgid))) {
        problems.push(`${relative(ROOT, poFile)}: placeholders differ ${JSON.stringify(key)}`);
      }
    }
  }
  return problems;
}

function messagesExtract() {
  const folder = mkdtempSync(join(tmpdir(), "translations-check-"));
  try {
    const pot = join(folder, COMMITTED_POT);
    execFileSync("pnpm", ["exec", "ttag", "extract", "--output", pot, ...EXTRACT_SOURCES], { cwd: SVELTE, stdio: "pipe" });
    return messagesRead(pot);
  } catch (error) {
    process.stderr.write(error.stderr ?? "");
    throw error;
  } finally {
    rmSync(folder, { recursive: true, force: true });
  }
}

/**
 * A gettext file's messages by context and English, without the header.
 * @param {string} path
 * @returns {Map<string, import("gettext-parser").GetTextTranslation>}
 */
function messagesRead(path) {
  const catalog = po.parse(readFileSync(path));
  const messages = new Map();
  for (const [context, byEnglish] of Object.entries(catalog.translations)) {
    for (const [english, message] of Object.entries(byEnglish)) {
      if (context === "" && english === "") continue;
      messages.set(messageKey(context, english), message);
    }
  }
  return messages;
}

/**
 * @param {string} context
 * @param {string} english
 */
function messageKey(context, english) {
  return context ? `${context}|${english}` : english;
}

/**
 * The messages in the pages missing from `file`, and those in it the pages
 * no longer have.
 * @param {string} file
 * @param {Map<string, import("gettext-parser").GetTextTranslation>} messages
 * @param {Map<string, import("gettext-parser").GetTextTranslation>} expected
 */
function messagesCompare(file, messages, expected) {
  const problems = [];
  for (const [key, message] of expected) {
    const found = messages.get(key);
    if (!found) problems.push(`${file}: missing ${JSON.stringify(key)}`);
    else if ((found.msgid_plural ?? "") !== (message.msgid_plural ?? "")) {
      problems.push(`${file}: plural changed for ${JSON.stringify(key)}`);
    }
  }
  for (const key of messages.keys()) {
    if (!expected.has(key)) problems.push(`${file}: no longer used ${JSON.stringify(key)}`);
  }
  return problems;
}

/**
 * @param {string} file
 * @param {Map<string, import("gettext-parser").GetTextTranslation>} messages
 * @param {Map<string, import("gettext-parser").GetTextTranslation>} expected
 */
function localeProblems(file, messages, expected) {
  const problems = messagesCompare(file, messages, expected);
  for (const [key, message] of messages) {
    if (!expected.has(key)) continue;
    const isFuzzy = message.comments?.flag?.split(",").some((flag) => flag.trim() === "fuzzy");
    if (isFuzzy) problems.push(`${file}: fuzzy ${JSON.stringify(key)}`);
    else if (message.msgstr.some((translation) => translation === "")) {
      problems.push(`${file}: untranslated ${JSON.stringify(key)}`);
    }
  }
  return problems;
}

/** @param {string} folder */
function localeFiles(folder) {
  return readdirSync(folder)
    .filter((file) => file.endsWith(".po"))
    .sort();
}
