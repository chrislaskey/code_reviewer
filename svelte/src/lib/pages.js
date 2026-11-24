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
  REVIEWS_NEW_PATH,
  REVIEWS_SHOW_PATH,
  SESSIONS_INDEX_PATH,
  SESSIONS_NEW_PATH,
  SESSIONS_SHOW_PATH,
  SUPPORT_INDEX_PATH,
  TRACES_NEW_PATH,
  TRACES_SHOW_PATH,
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
  [REVIEWS_NEW_PATH]: () => import("../reviews/ReviewsNewPage.svelte"),
  [REVIEWS_SHOW_PATH]: () => import("../reviews/ReviewsShowPage.svelte"),
  [SESSIONS_INDEX_PATH]: () => import("../sessions/SessionsIndexPage.svelte"),
  [SESSIONS_NEW_PATH]: () => import("../sessions/SessionsNewPage.svelte"),
  [SESSIONS_SHOW_PATH]: () => import("../sessions/SessionsShowPage.svelte"),
  [TRACES_NEW_PATH]: () => import("../traces/TracesNewPage.svelte"),
  [TRACES_SHOW_PATH]: () => import("../traces/TracesShowPage.svelte"),
};

// The feature each app's pages require, by the path they are under: when
// it is disabled in the server's settings, the nav shows no tab for them
// and the router redirects their paths to Home.
/** @type {Record<string, string>} */
export const APP_PAGES_REQUIRED_FEATURES = {
  [SUPPORT_INDEX_PATH]: "support",
  [FORUM_INDEX_PATH]: "forum",
};
