<!-- Around every page: a new instance of the page for every address, so a
     page reads its path and query once, when it mounts. The router keeps
     the instance when only the query or a path parameter changes, so this
     keys the page on both. -->
<script>
  import { searchParams } from "sv-router";
  import { pathParams } from "../router.js";

  let { children } = $props();
</script>

{#key `${JSON.stringify(pathParams())}?${searchParams.toString()}`}
  {@render children()}
{/key}
