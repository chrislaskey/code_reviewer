// Writes the Rust catalog's messages to core/translations/default.pot,
// `pnpm translations:extract`, see rust_translations.js.
import { mkdirSync, writeFileSync } from "node:fs";
import { resolve } from "node:path";
import { po } from "gettext-parser";
import { RUST_TRANSLATIONS, rustMessages } from "./rust_translations.js";

const messages = rustMessages();
const translations = { "": { "": { msgid: "", msgstr: [""] } } };
for (const [message, references] of messages) {
  translations[""][message] = { msgid: message, msgstr: [""], comments: { reference: references.join("\n") } };
}
const catalog = {
  charset: "utf-8",
  headers: { "Content-Type": "text/plain; charset=utf-8", "Plural-Forms": "nplurals=2; plural=(n!=1);" },
  translations,
};
mkdirSync(RUST_TRANSLATIONS, { recursive: true });
writeFileSync(resolve(RUST_TRANSLATIONS, "default.pot"), po.compile(catalog, { foldLength: 0 }));
console.log(`✔ ${messages.size} Rust messages extracted to core/translations/default.pot`);
