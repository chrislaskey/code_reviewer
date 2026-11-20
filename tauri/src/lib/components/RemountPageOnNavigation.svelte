<!-- Around every page: a new instance of the page for every address, so a
     page reads its query once, when it mounts, as it did when every page
     was its own HTML file. The router keeps the instance while the route
     stays the same: when only the query changes (the Review tab from a
     filled-in new review page) or only a path parameter does (one session's
     page to another's), so this keys the page on the path and the query. -->
<script>
  import { searchParams } from "sv-router";
  import { route } from "../router.js";

  let { children } = $props();
</script>

{#key `${route.pathname}?${searchParams.toString()}`}
  {@render children()}
{/key}
