<!-- A bar above the content for its options. -->
<script>
  /** @type {{ class?: any, children: import("svelte").Snippet }} */
  let { class: class_, children } = $props();
</script>

<div class={["flex shrink-0 flex-wrap items-center gap-x-6 gap-y-2 border-b border-base-300 px-6 py-2 max-sm:gap-x-3", class_]}>
  {@render children()}
</div>
