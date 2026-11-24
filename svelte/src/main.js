import { mount } from "svelte";
import { Router } from "sv-router";
import { isTauri } from "@tauri-apps/api/core";
// Registers the pages with the router before it mounts.
import "./lib/router.js";
import { translationsStart } from "./lib/translations.js";
import { onSessionNotificationClicked } from "./lib/core/index.js";
import { navigateToUrl } from "./lib/router.js";
import { sessionsShowUrl } from "./lib/routes.js";

// A clicked session notification opens the session's page.
onSessionNotificationClicked((name) => navigateToUrl(sessionsShowUrl(name)));

// Where the pages run, for the styles that keep clear of the window chrome
// (styles.css): "desktop" under the Tauri window's title bar, "mobile"
// under a phone's status bar, "web" in a browser.
document.documentElement.dataset.app = isTauri() ? (navigator.maxTouchPoints > 0 ? "mobile" : "desktop") : "web";

// The phone app does not pinch-zoom, as a native app does not, nor does
// the iPhone's sign-in sheet over it, whose pages carry app_scheme; a
// browser still does, for the people who need it.
const inSignInSheet = new URLSearchParams(window.location.search).has("app_scheme");
if (document.documentElement.dataset.app === "mobile" || inSignInSheet) {
  document
    .querySelector('meta[name="viewport"]')
    ?.setAttribute("content", "width=device-width, initial-scale=1.0, maximum-scale=1.0, user-scalable=no, viewport-fit=cover");
}

// In the language chosen, before the first page draws its text
translationsStart().then(() => mount(Router, { target: document.getElementById("app") }));
