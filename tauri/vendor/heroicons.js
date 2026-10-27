// The same "hero-#{ICON}" classes as Phoenix's assets/vendor/heroicons.js,
// reading the icons from the heroicons npm package instead of mix deps.

import fs from "node:fs";
import path from "node:path";
import plugin from "tailwindcss/plugin";

const ICONS_DIRECTORY = path.join(import.meta.dirname, "../node_modules/heroicons");

const STYLES = [
  ["", "24/outline"],
  ["-solid", "24/solid"],
  ["-mini", "20/solid"],
  ["-micro", "16/solid"],
];

export default plugin(function ({ matchComponents, theme }) {
  const values = {};
  for (const [suffix, directory] of STYLES) {
    for (const file of fs.readdirSync(path.join(ICONS_DIRECTORY, directory))) {
      const name = path.basename(file, ".svg") + suffix;
      values[name] = { name, fullPath: path.join(ICONS_DIRECTORY, directory, file) };
    }
  }

  matchComponents(
    {
      hero: ({ name, fullPath }) => {
        const content = encodeURIComponent(fs.readFileSync(fullPath, "utf8").replace(/\r?\n|\r/g, ""));
        let size = theme("spacing.6");
        if (name.endsWith("-mini")) size = theme("spacing.5");
        else if (name.endsWith("-micro")) size = theme("spacing.4");
        return {
          [`--hero-${name}`]: `url('data:image/svg+xml;utf8,${content}')`,
          "-webkit-mask": `var(--hero-${name})`,
          mask: `var(--hero-${name})`,
          "mask-repeat": "no-repeat",
          "background-color": "currentColor",
          "vertical-align": "middle",
          display: "inline-block",
          width: size,
          height: size,
        };
      },
    },
    { values },
  );
});
