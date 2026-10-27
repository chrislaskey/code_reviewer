<!-- Added and removed line counts of a diff. -->
<script>
  /** @type {{ lines: any[] }} */
  let { lines } = $props();

  let added = $derived(lines.filter((line) => line.kind === "added").length);
  let removed = $derived(lines.filter((line) => line.kind === "removed").length);
</script>

<span class="ml-auto flex gap-2 text-[11px]">
  {#if added > 0}<span class="text-emerald-600 dark:text-emerald-400">+{added}</span>{/if}
  {#if removed > 0}<span class="text-rose-600 dark:text-rose-400">−{removed}</span>{/if}
</span>
