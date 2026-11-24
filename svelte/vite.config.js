import { existsSync, mkdirSync, readFileSync, readdirSync, statSync, writeFileSync } from "node:fs";
import { basename, dirname, join, resolve, sep } from "node:path";
import sirv from "sirv";
import { defineConfig } from "vite";
import { svelte } from "@sveltejs/vite-plugin-svelte";
import tailwindcss from "@tailwindcss/vite";
import { po } from "gettext-parser";
import { PUBLIC_DEFAULT_LOCALE, publicCatalogs, publicPageTranslated } from "./scripts/public_pages_translations.js";

// The theme script sets `data-theme` on <html> before the first paint. The
// `<!-- theme-script -->` marker in an HTML entry becomes an inline classic
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

// The public website (src/public): plain HTML pages, one file each, built
// as entries of their own to dist/public. The web server (../axum) answers
// `/about` with `public/about.html` and `/` with `public/index.html`; the
// dev server does the same here. The
// `<!-- public-header -->` and `<!-- public-footer -->` markers become the
// files in src/public/partials. Each page is in every language of
// translations/public too, `/es/about` for `es`: the dev server
// translates a page as it answers it, and a build writes each language's
// copy to dist/public/<locale>/ once Vite has written the English, see
// scripts/public_pages_translations.js. See docs/design/public-pages.md.
const PUBLIC_PAGES = resolve(import.meta.dirname, "src/public");
const PUBLIC_PARTIALS = { "public-header": "header.html", "public-footer": "footer.html" };

function publicPageNames() {
  return readdirSync(PUBLIC_PAGES)
    .filter((file) => file.endsWith(".html"))
    .map((file) => file.slice(0, -".html".length));
}

function publicPages() {
  return {
    name: "public-pages",
    transformIndexHtml(html, context) {
      for (const [marker, file] of Object.entries(PUBLIC_PARTIALS)) {
        html = html.replace(`<!-- ${marker} -->`, () => readFileSync(resolve(PUBLIC_PAGES, "partials", file), "utf8"));
      }
      if (!context.server || !context.path.startsWith("/public/")) return html;
      const catalogs = publicCatalogs();
      const [first] = new URL(context.originalUrl ?? "/", "http://localhost").pathname.split("/").filter(Boolean);
      const locale = catalogs.has(first) ? first : PUBLIC_DEFAULT_LOCALE;
      const name = basename(context.path, ".html");
      return publicPageTranslated(html, { locale, name, pageNames: publicPageNames(), catalogs });
    },
    writeBundle(options, bundle) {
      const catalogs = publicCatalogs();
      const pageNames = publicPageNames();
      for (const fileName of Object.keys(bundle)) {
        if (!fileName.startsWith("public/") || !fileName.endsWith(".html")) continue;
        const name = basename(fileName, ".html");
        const english = resolve(options.dir, fileName);
        const html = readFileSync(english, "utf8");
        for (const locale of [PUBLIC_DEFAULT_LOCALE, ...catalogs.keys()]) {
          const file = locale === PUBLIC_DEFAULT_LOCALE ? english : resolve(dirname(english), locale, `${name}.html`);
          mkdirSync(dirname(file), { recursive: true });
          writeFileSync(file, publicPageTranslated(html, { locale, name, pageNames, catalogs }));
        }
      }
    },
    configureServer(server) {
      server.watcher.on("change", (file) => {
        if (file.startsWith(resolve(PUBLIC_PAGES, "partials"))) server.ws.send({ type: "full-reload" });
      });
      server.middlewares.use((request, _response, next) => {
        const { pathname, search } = new URL(request.url ?? "/", "http://localhost");
        const parts = pathname.split("/").filter(Boolean);
        if (parts.length > 0 && publicCatalogs().has(parts[0])) parts.shift();
        const name = parts.length === 0 ? "index" : parts.join("/");
        const isPublicPage = /^[\w-]+$/.test(name) && existsSync(resolve(PUBLIC_PAGES, `${name}.html`));
        if (isPublicPage) request.url = `/public/${name}.html${search}`;
        // The ops app's pages are its own entry; in dev, open to anyone
        if (pathname === "/ops" || pathname.startsWith("/ops/")) {
          const isPage = !/\.\w+$/.test(pathname);
          if (isPage) request.url = `/ops.html${search}`;
        }
        next();
      });
    },
  };
}

// A translations file (translations/*.po) imported as the catalog ttag's
// `addLocale` takes, without the fuzzy entries, whose English has changed
// since they were translated, so the page shows the English instead.
function translationFiles() {
  return {
    name: "translation-files",
    transform(source, id) {
      if (!id.endsWith(".po")) return null;
      const catalog = po.parse(source);
      for (const messages of Object.values(catalog.translations)) {
        for (const [key, message] of Object.entries(messages)) {
          const isFuzzy = message.comments?.flag?.split(",").some((flag) => flag.trim() === "fuzzy");
          if (isFuzzy) delete messages[key];
          else delete message.comments;
        }
      }
      return { code: `export default ${JSON.stringify(catalog)};`, map: null };
    },
  };
}

// Where `pnpm dev` sends `/api` requests: the web server (../axum).
// Inside the Tauri app the pages call commands instead, see src/lib/core/.
const WEB_SERVER_URL = process.env.WEB_SERVER_URL ?? "http://localhost:4000";

// `cargo tauri ios dev` and `android dev` set TAURI_DEV_HOST to this Mac's
// address on the network, so a phone or simulator reaches the dev server.
const TAURI_DEV_HOST = process.env.TAURI_DEV_HOST;

// One HTML entry per window: src/index.html, whose router
// (src/lib/router.js) shows the page at the address, and
// src/quick-picker.html for the desktop app's quick picker; and one per
// page of the public website. Files served as they are, the fonts, are in
// src/static.
//
// The ops app, src/ops.html, is a build of its own, `vite build --mode
// ops`, to dist-ops/ with its files under /ops/: the web server gives
// them to staff alone, and the Tauri app, which bundles dist/, never has
// them. `pnpm build` runs both. The dev server
// answers any other path with index.html, and so do the web server
// (../axum) and the Tauri app, so every page is reloadable. `pnpm build` writes dist/, which the web server
// serves and the Tauri app bundles.
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

export default defineConfig(({ mode }) => ({
  root: "src",
  base: mode === "ops" ? "/ops/" : "/",
  publicDir: "static",
  plugins: [themeScript(), artifacts(), publicPages(), translationFiles(), tailwindcss(), svelte()],
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
    host: TAURI_DEV_HOST || false,
    hmr: TAURI_DEV_HOST ? { protocol: "ws", host: TAURI_DEV_HOST, port: 1421 } : undefined,
    proxy: { "/api": WEB_SERVER_URL },
  },
  build:
    mode === "ops"
      ? {
          outDir: "../dist-ops",
          emptyOutDir: true,
          rolldownOptions: { input: { ops: resolve(import.meta.dirname, "src/ops.html") } },
        }
      : {
          outDir: "../dist",
          emptyOutDir: true,
          rolldownOptions: {
            input: {
              main: resolve(import.meta.dirname, "src/index.html"),
              quick_picker: resolve(import.meta.dirname, "src/quick-picker.html"),
              ...Object.fromEntries(
                publicPageNames().map((name) => [`public_${name}`, resolve(PUBLIC_PAGES, `${name}.html`)]),
              ),
            },
          },
        },
}));
