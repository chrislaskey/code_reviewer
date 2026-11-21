<!-- Around every page: a new instance of the page for every address, so a
     page reads its path and query once, when it mounts. The router keeps
     the instance when only the query or a path parameter changes, so this
     keys the page on both. Above it, what went wrong getting it ready. -->
<script>
  import { searchParams } from "sv-router";
  import { pageLoadError } from "../page_load_error.svelte.js";
  import { pathParams } from "../router.js";
  import { Alert } from "../components/index.js";

  let { children } = $props();
</script>

{#if pageLoadError.message}
  <Alert tone="error" role="alert" class="m-4">{pageLoadError.message}</Alert>
{/if}
{#key `${JSON.stringify(pathParams())}?${searchParams.toString()}`}
  {@render children()}
{/key}
