<!-- A page of the app: its column, with what the navigation has no place
     for yet above it (PageTopBar), unless `topBar` is false. -->
<script>
  import PageTopBar from "./PageTopBar.svelte";

  /** @type {{ topBar?: boolean, class?: any, children: import("svelte").Snippet }} */
  let { topBar = true, class: class_, children } = $props();
</script>

<main class={["page", class_]}>
  {#if topBar}
    <PageTopBar />
  {/if}
  {@render children()}
</main>
