<!-- A heading: the page's title at level 1, a section's at 2 (the
     default) or 3. -->
<script>
  /** @type {{ level?: 1 | 2 | 3, class?: any, children: import("svelte").Snippet }} */
  let { level = 2, class: class_, children } = $props();
</script>

<svelte:element this={`h${level}`} class={["heading", `heading-${level}`, class_]}>
  {@render children()}
</svelte:element>
