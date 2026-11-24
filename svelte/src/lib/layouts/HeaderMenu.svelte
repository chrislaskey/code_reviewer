<!-- A button in the app's header (AppHeader) that opens a panel below it,
     lined up with its right edge; the panel closes on the button, a click
     outside it, Escape and `close()`, which the panel's links call once
     chosen. The button's content is the `button` snippet. -->
<script>
  import Button from "../components/Button.svelte";

  /**
   * @typedef {Object} Props
   * @property {string} label the button's name for a screen reader
   * @property {any} [buttonClass]
   * @property {import("svelte").Snippet} button
   * @property {import("svelte").Snippet<[() => void]>} children the panel, given `close`
   */

  /** @type {Props} */
  let { label, buttonClass, button, children } = $props();

  let open = $state(false);
  /** @type {HTMLElement | undefined} */
  let element = $state();

  function close() {
    open = false;
  }

  /** @param {MouseEvent} event */
  function closeWhenClickedOutside(event) {
    if (open && element && !element.contains(/** @type {Node} */ (event.target))) open = false;
  }

  /** @param {KeyboardEvent} event */
  function closeOnEscape(event) {
    if (event.key === "Escape") open = false;
  }
</script>

<svelte:window onclick={closeWhenClickedOutside} onkeydown={closeOnEscape} />

<div class="relative" bind:this={element}>
  <Button
    variant="ghost"
    size="sm"
    class={buttonClass}
    aria-label={label}
    title={label}
    aria-expanded={open}
    onclick={() => (open = !open)}
  >
    {@render button()}
  </Button>
  {#if open}
    <div class="header-menu-panel">
      {@render children(close)}
    </div>
  {/if}
</div>
