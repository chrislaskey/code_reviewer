<!-- A dialog's buttons, along its bottom right. -->
<script>
  /** @type {{ children: import("svelte").Snippet }} */
  let { children } = $props();
</script>

<div class="flex flex-wrap justify-end gap-2">
  {@render children()}
</div>
