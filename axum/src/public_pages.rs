//! The public website: the built HTML pages in `public/` of the Svelte
//! build, one file each, see `docs/design/public-pages.md`. A page
//! answers at its file's name, `public/about.html` at `/about`, and the
//! home page, `public/index.html`, at `/`. Each in another language is
//! in its folder, `public/es/about.html` at `/es/about` and
//! `public/es/index.html` at `/es`, for each of the languages the pages
//! are in, `helpers::locales::SUPPORTED`. Every other path stays the
//! app's.

use std::fs;
use std::path::Path;

use axum::Router;
use axum::routing::get_service;
use tower_http::services::ServeFile;

use app_core::helpers::locales;

const HOME_PAGE: &str = "index";

/// The routes of the public pages the build has, in each language it
/// has them in; none when it has no `public/`.
pub fn router(svelte_dist: &Path) -> Router {
    let public_pages = svelte_dist.join("public");
    let mut router = Router::new();
    let languages = locales::SUPPORTED.iter().map(|locale| match *locale {
        locales::DEFAULT => (String::new(), public_pages.clone()),
        locale => (format!("/{locale}"), public_pages.join(locale)),
    });
    for (prefix, folder) in languages {
        for name in page_names(&folder) {
            let path = match (name.as_str(), prefix.as_str()) {
                (HOME_PAGE, "") => "/".to_string(),
                (HOME_PAGE, prefix) => prefix.to_string(),
                (name, prefix) => format!("{prefix}/{name}"),
            };
            let page = folder.join(format!("{name}.html"));
            router = router.route(&path, get_service(ServeFile::new(page)));
        }
    }
    router
}

/// The names of the `.html` files in the folder, without the extension.
fn page_names(public_pages: &Path) -> Vec<String> {
    let Ok(entries) = fs::read_dir(public_pages) else {
        return Vec::new();
    };
    entries
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.path())
        .filter(|path| {
            path.extension()
                .is_some_and(|extension| extension == "html")
        })
        .filter_map(|path| Some(path.file_stem()?.to_str()?.to_string()))
        .collect()
}
