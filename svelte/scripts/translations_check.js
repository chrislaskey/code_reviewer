// Checks the translations, `pnpm translations:check`: that the pages'
// markup writes its English through ttag (untranslated_text.js); then,
// with the messages extracted afresh, that translations/default.pot has
// them all and no others, and that every translations/<locale>.po
// translates each of them, none fuzzy. Prints what is wrong and exits 1.
import { execFileSync } from "node:child_process";
import { mkdtempSync, readFileSync, readdirSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { po } from "gettext-parser";
import { untranslatedText } from "./untranslated_text.js";

const SVELTE = resolve(import.meta.dirname, "..");
const TRANSLATIONS = resolve(SVELTE, "translations");
const COMMITTED_POT = "default.pot";
// The same sources as package.json's translations:extract
const IGNORED = "src/ops";
const EXTRACT_SOURCES = ["--ignore", `${IGNORED}/**`, "src"];

const untranslated = untranslatedText(SVELTE, resolve(SVELTE, "src"), [IGNORED]);
const extracted = messagesExtract();
const problems = [
  ...messagesCompare(COMMITTED_POT, messagesRead(resolve(TRANSLATIONS, COMMITTED_POT)), extracted),
  ...localeFiles().flatMap((file) => localeProblems(file, messagesRead(resolve(TRANSLATIONS, file)), extracted)),
];

if (untranslated.length > 0) {
  for (const problem of untranslated) console.error(problem);
  console.error(
    "\nWrite these through ttag, `{t`...`}`, or put `<!-- untranslated: why -->` before one that stays as it is.\n",
  );
}
if (problems.length > 0) {
  for (const problem of problems) console.error(`translations/${problem}`);
  console.error(
    "\nRun `pnpm translations:update`, then translate what is untranslated or fuzzy in each .po and remove its fuzzy flag.",
  );
}
if (untranslated.length > 0 || problems.length > 0) process.exit(1);
console.log(`Translations: ${extracted.size} messages, ${localeFiles().length} languages, all translated.`);

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

function localeFiles() {
  return readdirSync(TRANSLATIONS)
    .filter((file) => file.endsWith(".po"))
    .sort();
}
