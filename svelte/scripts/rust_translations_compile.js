// Compiles each core/translations/<locale>.po to the <locale>.mo Rust
// reads, `pnpm translations:update`, see rust_translations.js.
import { writeFileSync } from "node:fs";
import { rustCompiled, rustPoFiles } from "./rust_translations.js";

for (const poFile of rustPoFiles()) {
  writeFileSync(poFile.replace(/\.po$/, ".mo"), rustCompiled(poFile));
}
console.log(`✔ ${rustPoFiles().length} Rust catalogs compiled to core/translations/*.mo`);
