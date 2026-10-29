import { mount } from "svelte";
import { Router } from "sv-router";
// Registers the pages with the router before it mounts.
import "./lib/router.js";

mount(Router, { target: document.getElementById("app") });
