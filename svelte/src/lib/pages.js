// The app's pages, each at its path from routes.js: the router (router.js)
// shows them beside the template's own. A page is loaded the first time it
// is visited, so each page's code stays a chunk of its own. Every app
// edits this list, a line per page, and may change where a person lands
// after signing in, pageAfterSignInUrl.
import {
  FILES_EDIT_PATH,
  FILES_INDEX_PATH,
  FILES_NEW_PATH,
  FILES_SHOW_PATH,
  FORUM_INDEX_PATH,
  HOME_PATH,
  POSTS_EDIT_PATH,
  POSTS_INDEX_PATH,
  POSTS_NEW_PATH,
  POSTS_SHOW_PATH,
  PROFILE_EDIT_PATH,
  PUBLIC_HOME_PATH,
  SUPPORT_INDEX_PATH,
  homeUrl,
} from "./routes.js";

/** @type {Record<string, () => Promise<unknown>>} */
export const APP_PAGES = {
  [PUBLIC_HOME_PATH]: () => import("../home/HomePage.svelte"),
  [HOME_PATH]: () => import("../home/HomePage.svelte"),
  [POSTS_INDEX_PATH]: () => import("../posts/PostsIndexPage.svelte"),
  [POSTS_NEW_PATH]: () => import("../posts/PostsNewPage.svelte"),
  [POSTS_SHOW_PATH]: () => import("../posts/PostsShowPage.svelte"),
  [POSTS_EDIT_PATH]: () => import("../posts/PostsEditPage.svelte"),
  [FILES_INDEX_PATH]: () => import("../files/FilesIndexPage.svelte"),
  [FILES_NEW_PATH]: () => import("../files/FilesNewPage.svelte"),
  [FILES_SHOW_PATH]: () => import("../files/FilesShowPage.svelte"),
  [FILES_EDIT_PATH]: () => import("../files/FilesEditPage.svelte"),
  [PROFILE_EDIT_PATH]: () => import("../profile/ProfileEditPage.svelte"),
  [SUPPORT_INDEX_PATH]: () => import("../support/SupportIndexPage.svelte"),
  [FORUM_INDEX_PATH]: () => import("../forum/ForumIndexPage.svelte"),
};

// The feature each app's pages require, by the path they are under: when
// it is disabled in the server's settings, the nav shows no tab for them
// and the router redirects their paths to Home.
/** @type {Record<string, string>} */
export const APP_PAGES_REQUIRED_FEATURES = {
  [SUPPORT_INDEX_PATH]: "support",
  [FORUM_INDEX_PATH]: "forum",
};

/**
 * Where a person lands once signed in, signed up, or started as a guest:
 * Home. An app may send them elsewhere, for everyone or person by person.
 * @returns {string}
 */
export function pageAfterSignInUrl() {
  return homeUrl();
}
