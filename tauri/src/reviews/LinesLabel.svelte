<!-- Lines added and removed. -->
<script>
  /** @type {{ added: number, removed: number, class?: any }} */
  let { added, removed, class: class_ = "" } = $props();
</script>

<span class={["font-mono text-[11px]", class_]} title="Lines added and removed">
  <span class="text-success">+{added}</span>
  <span class="text-error">−{removed}</span>
</span>
