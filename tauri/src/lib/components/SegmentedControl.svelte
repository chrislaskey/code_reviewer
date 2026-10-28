<!-- One of a few choices, like a tab bar. Small in toolbars, medium in forms. -->
<script>
  /** @type {{ value: string, options: { value: string, label: string }[], onchange: (value: string) => void, label?: string, size?: "small" | "medium" }} */
  let { value, options, onchange, label, size = "small" } = $props();

  const SIZES = {
    small: { label: "text-[11px]", group: "text-xs", button: "px-3 py-1" },
    medium: { label: "text-sm", group: "text-sm", button: "px-4 py-2" },
  };

  let classes = $derived(SIZES[size]);
</script>

<div class="flex items-center gap-2">
  {#if label}<span class={[classes.label, "text-base-content/60"]}>{label}</span>{/if}
  <div class={["inline-flex overflow-hidden rounded-lg border border-base-300", classes.group]}>
    {#each options as option (option.value)}
      <button
        type="button"
        onclick={() => onchange(option.value)}
        class={[
          classes.button,
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
