// "lucide-#{ICON}" classes for the icons heroicons lacks, such as a push
// pin, drawn the same way as vendor/heroicons.js: a mask in currentColor.

import fs from "node:fs";
import path from "node:path";
import plugin from "tailwindcss/plugin";

const ICONS_DIRECTORY = path.join(import.meta.dirname, "../node_modules/lucide-static/icons");

export default plugin(function ({ matchComponents, theme }) {
  const values = {};
  for (const file of fs.readdirSync(ICONS_DIRECTORY)) {
    const name = path.basename(file, ".svg");
    values[name] = { name, fullPath: path.join(ICONS_DIRECTORY, file) };
  }

  matchComponents(
    {
      lucide: ({ name, fullPath }) => {
        const content = encodeURIComponent(fs.readFileSync(fullPath, "utf8").replace(/\r?\n|\r/g, ""));
        return {
          [`--lucide-${name}`]: `url('data:image/svg+xml;utf8,${content}')`,
          "-webkit-mask": `var(--lucide-${name})`,
          mask: `var(--lucide-${name})`,
          "mask-repeat": "no-repeat",
          "mask-size": "100% 100%",
          "background-color": "currentColor",
          "vertical-align": "middle",
          display: "inline-block",
          width: theme("spacing.6"),
          height: theme("spacing.6"),
        };
      },
    },
    { values },
  );
});
