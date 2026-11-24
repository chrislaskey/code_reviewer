<!-- A page of the app: its column, wider with `wide`, for a page of
     tables, code or long lines. -->
<script>
  /** @type {{ wide?: boolean, class?: any, children: import("svelte").Snippet }} */
  let { wide = false, class: class_, children } = $props();
</script>

<main class={["page", wide && "page-wide", class_]}>
  {@render children()}
</main>
