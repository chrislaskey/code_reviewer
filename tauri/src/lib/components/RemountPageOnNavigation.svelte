<!-- Around every page: a new instance of the page for every address, so a
     page reads its query once, when it mounts, as it did when every page
     was its own HTML file. The router keeps the instance when only the
     query changes (the Review tab from a filled-in new review page), so this
     keys the page on the query; a change of path changes the page anyway. -->
<script>
  import { searchParams } from "sv-router";

  let { children } = $props();
</script>

{#key searchParams.toString()}
  {@render children()}
{/key}
