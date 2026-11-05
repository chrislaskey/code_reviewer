import { mount } from "svelte";
import { Router } from "sv-router";
import NavigationHistoryButtons from "./lib/components/NavigationHistoryButtons.svelte";
// Registers the pages with the router before it mounts.
import "./lib/router.js";

mount(Router, { target: document.getElementById("app") });
// Outside the router, so the buttons stay put while the pages swap.
mount(NavigationHistoryButtons, { target: document.body });
