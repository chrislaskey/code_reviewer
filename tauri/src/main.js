import { mount } from "svelte";
import { Router } from "sv-router";
import { onSessionNotificationClicked } from "./lib/backend.js";
import NavigationHistoryButtons from "./lib/components/NavigationHistoryButtons.svelte";
// Also registers the pages with the router before it mounts.
import { navigateToUrl } from "./lib/router.js";
import { sessionShowUrl } from "./lib/routes.js";
import { followThemeWithWebviewBackground } from "./lib/webview_background.js";

mount(Router, { target: document.getElementById("app") });
// Outside the router, so the buttons stay put while the pages swap.
mount(NavigationHistoryButtons, { target: document.body });
followThemeWithWebviewBackground();
onSessionNotificationClicked((name) => navigateToUrl(sessionShowUrl(name)));
