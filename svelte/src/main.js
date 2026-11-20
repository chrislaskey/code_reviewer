import { mount } from "svelte";
import { Router } from "sv-router";
import { isTauri } from "@tauri-apps/api/core";
// Registers the pages with the router before it mounts.
import "./lib/router.js";

// Where the pages run, for the styles that keep clear of the window chrome
// (styles.css): "desktop" under the Tauri window's title bar, "mobile"
// under a phone's status bar, "web" in a browser.
document.documentElement.dataset.app = isTauri() ? (navigator.maxTouchPoints > 0 ? "mobile" : "desktop") : "web";

// The phone app does not pinch-zoom, as a native app does not; a browser
// still does, for the people who need it.
if (document.documentElement.dataset.app === "mobile") {
  document
    .querySelector('meta[name="viewport"]')
    ?.setAttribute("content", "width=device-width, initial-scale=1.0, maximum-scale=1.0, user-scalable=no, viewport-fit=cover");
}

mount(Router, { target: document.getElementById("app") });
