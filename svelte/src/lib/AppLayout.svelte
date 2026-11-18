<!-- The router's layout, around every page: the navigation (AppNav), which
     outlives the pages so it never redraws as they swap, beside the page. -->
<script>
  import AppNav from "./AppNav.svelte";
  import RemountPageOnNavigation from "./RemountPageOnNavigation.svelte";

  let { children } = $props();
</script>

<AppNav />
<div class="app-layout">
  <RemountPageOnNavigation>
    {@render children()}
  </RemountPageOnNavigation>
</div>
