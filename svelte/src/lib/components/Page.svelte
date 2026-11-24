<!-- A page of the app: its column. -->
<script>
  /** @type {{ class?: any, children: import("svelte").Snippet }} */
  let { class: class_, children } = $props();
</script>

<main class={["page", class_]}>
  {@render children()}
</main>
