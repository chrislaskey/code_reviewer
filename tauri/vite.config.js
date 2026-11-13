import { existsSync, readdirSync, readFileSync, statSync } from "node:fs";
import { join, resolve, sep } from "node:path";
import sirv from "sirv";
import { defineConfig } from "vite";
import { svelte } from "@sveltejs/vite-plugin-svelte";
import tailwindcss from "@tailwindcss/vite";

// The theme script sets `data-theme` on <html> before the first paint. The
// `<!-- theme-script -->` marker in index.html becomes an inline classic
// script with the file's contents, in dev and in builds.
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

// The artifacts (artifacts/README.md): files made while working, such as
// screenshots and previews, under artifacts/<project>/<worktree>/, at
// /artifacts/ on the dev server only. A folder's address shows the index,
// which lists the files below it from /artifacts/files.json, newest first.
const ARTIFACTS_DIRECTORY = resolve(import.meta.dirname, "../artifacts");
const ARTIFACTS_INDEX = join(ARTIFACTS_DIRECTORY, "index.html");

function listArtifacts() {
  return readdirSync(ARTIFACTS_DIRECTORY, { recursive: true })
    .filter((path) => path.includes("/") && !path.split("/").some((part) => part.startsWith(".")))
    .map((path) => ({ path, stats: statSync(join(ARTIFACTS_DIRECTORY, path)) }))
    .filter(({ stats }) => stats.isFile())
    .map(({ path, stats }) => ({ path, size: stats.size, modified: stats.mtimeMs }))
    .sort((a, b) => b.modified - a.modified);
}

function isArtifactsFolder(url) {
  let path;
  try {
    path = resolve(ARTIFACTS_DIRECTORY, "." + decodeURIComponent(url.split("?")[0]));
  } catch {
    return false;
  }
  if (path !== ARTIFACTS_DIRECTORY && !path.startsWith(ARTIFACTS_DIRECTORY + sep)) return false;
  return existsSync(path) && statSync(path).isDirectory();
}

function artifacts() {
  return {
    name: "artifacts",
    configureServer(server) {
      const serveArtifact = sirv(ARTIFACTS_DIRECTORY, { dev: true });
      server.middlewares.use("/artifacts", (request, response) => {
        if (request.url === "/files.json") {
          response.setHeader("Content-Type", "application/json");
          response.end(JSON.stringify(listArtifacts()));
          return;
        }
        if (isArtifactsFolder(request.url)) {
          response.setHeader("Content-Type", "text/html; charset=utf-8");
          response.end(readFileSync(ARTIFACTS_INDEX));
          return;
        }
        serveArtifact(request, response, () => {
          response.statusCode = 404;
          response.end("No such artifact");
        });
      });
    },
  };
}

// Host names besides localhost the dev server answers to, comma separated,
// such as this machine's names on a tailnet (bin/dev sets them).
const ALLOWED_HOSTS = (process.env.DEV_SERVER_ALLOWED_HOSTS ?? "")
  .split(",")
  .map((host) => host.trim())
  .filter(Boolean);

// The dev server listens on PORT when set (the app's sessions set one per
// worktree, and bin/dev points Tauri at it), else on 1420.
const PORT = Number(process.env.PORT) || 1420;

// One HTML entry, src/index.html, for every page: the router in
// src/lib/router.js shows the page at the address. The dev server answers
// any path with it, and so does the bundled app (Tauri's asset protocol
// falls back to index.html). `cargo tauri dev` loads the dev server (see
// `build.devUrl` in src-tauri/tauri.conf.json, overridden by bin/dev), and
// `cargo tauri build` bundles `dist/`.
export default defineConfig({
  root: "src",
  plugins: [themeScript(), artifacts(), tailwindcss(), svelte()],
  clearScreen: false,
  server: {
    port: PORT,
    strictPort: true,
    allowedHosts: ALLOWED_HOSTS,
    watch: { ignored: ["**/src-tauri/**"] },
  },
  build: {
    outDir: "../dist",
    emptyOutDir: true,
  },
});
