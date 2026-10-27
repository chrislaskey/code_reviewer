import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { defineConfig } from "vite";
import { svelte } from "@sveltejs/vite-plugin-svelte";
import tailwindcss from "@tailwindcss/vite";

// The theme script sets `data-theme` on <html> before the first paint. Each
// page's `<!-- theme-script -->` marker becomes an inline classic script with
// the file's contents, in dev and in builds. A page without the marker is
// left alone.
const THEME_SCRIPT = resolve(import.meta.dirname, "src/lib/theme_script.js");
const THEME_SCRIPT_MARKER = "<!-- theme-script -->";

function themeScript() {
  return {
    name: "theme-script",
    transformIndexHtml(html) {
      return html.replace(THEME_SCRIPT_MARKER, () => `<script>\n${readFileSync(THEME_SCRIPT, "utf8")}</script>`);
    },
    configureServer(server) {
      server.watcher.add(THEME_SCRIPT);
      server.watcher.on("change", (file) => {
        if (file === THEME_SCRIPT) server.ws.send({ type: "full-reload" });
      });
    },
  };
}

// The dev server listens on PORT when set (the app's sessions set one per
// worktree, and bin/dev points Tauri at it), else on 1420.
const PORT = Number(process.env.PORT) || 1420;

// One HTML entry per page. `cargo tauri dev` loads the dev server (see
// `build.devUrl` in src-tauri/tauri.conf.json, overridden by bin/dev), and
// `cargo tauri build` bundles `dist/`.
export default defineConfig({
  root: "src",
  plugins: [themeScript(), tailwindcss(), svelte()],
  clearScreen: false,
  server: {
    port: PORT,
    strictPort: true,
    watch: { ignored: ["**/src-tauri/**"] },
  },
  build: {
    outDir: "../dist",
    emptyOutDir: true,
    rollupOptions: {
      input: {
        home: resolve(import.meta.dirname, "src/index.html"),
        reviews_new: resolve(import.meta.dirname, "src/reviews/new.html"),
        reviews_show: resolve(import.meta.dirname, "src/reviews/show.html"),
        sessions_index: resolve(import.meta.dirname, "src/sessions/index.html"),
        sessions_new: resolve(import.meta.dirname, "src/sessions/new.html"),
      },
    },
  },
});
