// The ops app: the staff's tools, on the web only, an entry of its own
// (ops.html) so its code is not in what the app's pages load. One page
// so far, so no router yet. See docs/design/multi-app.md.
import { mount } from "svelte";
import OpsHomePage from "./OpsHomePage.svelte";

const target = document.getElementById("app");
if (target) mount(OpsHomePage, { target });
