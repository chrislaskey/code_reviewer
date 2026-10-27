<!-- The two buttons that open or close every ledger and code block. -->
<script>
  /** @type {{ oncommand: (open: boolean) => void }} */
  let { oncommand } = $props();

  const BUTTON = "px-3 py-1 bg-base-100 text-base-content/60 hover:text-base-content";
</script>

<div class="inline-flex overflow-hidden rounded-lg border border-base-300 text-xs">
  <button type="button" onclick={() => oncommand(true)} class={BUTTON}>Open all</button>
  <button type="button" onclick={() => oncommand(false)} class={[BUTTON, "border-l border-base-300"]}>Close all</button>
</div>
