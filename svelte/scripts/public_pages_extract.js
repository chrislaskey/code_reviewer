// Writes the public pages' messages to translations/public/default.pot,
// `pnpm translations:extract`, see public_pages_translations.js.
import { mkdirSync, writeFileSync } from "node:fs";
import { resolve } from "node:path";
import { po } from "gettext-parser";
import { PUBLIC_TRANSLATIONS, publicPagesMessages } from "./public_pages_translations.js";

const translations = { "": { "": { msgid: "", msgstr: [""] } } };
for (const [message, references] of publicPagesMessages()) {
  translations[""][message] = { msgid: message, msgstr: [""], comments: { reference: references.join("\n") } };
}
const catalog = {
  charset: "utf-8",
  headers: { "Content-Type": "text/plain; charset=utf-8", "Plural-Forms": "nplurals=2; plural=(n!=1);" },
  translations,
};
mkdirSync(PUBLIC_TRANSLATIONS, { recursive: true });
const pot = resolve(PUBLIC_TRANSLATIONS, "default.pot");
writeFileSync(pot, po.compile(catalog, { foldLength: 0 }));
console.log(`✔ ${publicPagesMessages().size} public pages' messages extracted to translations/public/default.pot`);
