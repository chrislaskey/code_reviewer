<!-- One choice in a ChoiceMenu: its label and a sentence saying what it
     does. A link when given `href`, else a button. -->
<script>
  /**
   * @typedef {Object} Props
   * @property {string} label
   * @property {string} explanation
   * @property {string} [href]
   * @property {boolean} [disabled]
   * @property {(event: MouseEvent) => void} [onclick]
   * @property {import("svelte").Snippet<[]>} [children]
   */

  /** @type {Props} */
  let { label, explanation, href, disabled = false, onclick, children } = $props();

  const CLASSES = "flex flex-col items-start gap-0.5";
</script>

<li class:menu-disabled={disabled}>
  {#if href && !disabled}
    <a {href} class="{CLASSES} text-base-content no-underline" {onclick}>
      <span class="choice">{label}</span>
      <span class="explanation whitespace-normal">{explanation} {@render children?.()}</span>
    </a>
  {:else}
    <button type="button" class={CLASSES} {disabled} {onclick}>
      <span class="choice">{label}</span>
      <span class="explanation whitespace-normal">{explanation} {@render children?.()}</span>
    </button>
  {/if}
</li>
