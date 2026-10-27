<!-- Lines added and removed. -->
<script>
  /** @type {{ added: number, removed: number, class?: any }} */
  let { added, removed, class: class_ = "" } = $props();
</script>

<span class={["font-mono text-[11px]", class_]} title="Lines added and removed">
  <span class="text-emerald-600 dark:text-emerald-400">+{added}</span>
  <span class="text-rose-600 dark:text-rose-400">−{removed}</span>
</span>
