// The app's pages, each at its path from routes.js: the router (router.js)
// shows them beside the template's own. A page is loaded the first time it
// is visited, so each page's code stays a chunk of its own. Every app
// edits this list, a line per page.
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

// The server feature each app's pages need, by the path they are under:
// with it off in the server's settings, the nav shows no tab for them and
// the router sends their paths home.
/** @type {Record<string, string>} */
export const APP_PAGES_FEATURES = {
  [SUPPORT_INDEX_PATH]: "support",
  [FORUM_INDEX_PATH]: "forum",
};
