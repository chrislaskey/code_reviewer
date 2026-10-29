<!-- A bar above the content for its options. -->
<script>
  /** @type {{ children: import("svelte").Snippet }} */
  let { children } = $props();
</script>

<div class="flex shrink-0 flex-wrap items-center gap-x-6 gap-y-2 border-b border-base-300 px-6 py-2">
  {@render children()}
</div>
