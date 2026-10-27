<!-- One of a few choices, like a tab bar. -->
<script>
  /** @type {{ value: string, options: { value: string, label: string }[], onchange: (value: string) => void, label?: string }} */
  let { value, options, onchange, label } = $props();
</script>

<div class="flex items-center gap-2">
  {#if label}<span class="text-[11px] text-base-content/60">{label}</span>{/if}
  <div class="inline-flex overflow-hidden rounded-lg border border-base-300 text-xs">
    {#each options as option (option.value)}
      <button
        type="button"
        onclick={() => onchange(option.value)}
        class={[
          "px-3 py-1",
          option.value === value
            ? "bg-base-content font-semibold text-base-100"
            : "bg-base-100 text-base-content/60 hover:text-base-content",
        ]}
      >
        {option.label}
      </button>
    {/each}
  </div>
</div>
