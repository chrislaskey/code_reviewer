<!-- A button in the app's header (AppHeader) that opens a panel below it,
     lined up with its right edge; the panel closes on the button, a click
     outside it, Escape and `close()`, which the panel's links call once
     chosen. The button's content is the `button` snippet. Given
     `ondoubletap`, a second tap soon after the first closes the panel the
     first opened and calls it instead. -->
<script>
  import Button from "../components/Button.svelte";

  /**
   * @typedef {Object} Props
   * @property {string} label the button's name for a screen reader
   * @property {any} [buttonClass]
   * @property {import("svelte").Snippet} button
   * @property {import("svelte").Snippet<[() => void]>} children the panel, given `close`
   * @property {() => void} [ondoubletap]
   */

  /** @type {Props} */
  let { label, buttonClass, button, children, ondoubletap } = $props();

  /** How soon a second tap must follow the first to be a double tap. */
  const DOUBLE_TAP_MILLISECONDS = 300;

  let open = $state(false);
  /** @type {HTMLElement | undefined} */
  let element = $state();
  let lastTapAt = 0;

  function close() {
    open = false;
  }

  function toggleOrDoubleTap() {
    const tappedAt = Date.now();
    if (ondoubletap && tappedAt - lastTapAt < DOUBLE_TAP_MILLISECONDS) {
      lastTapAt = 0;
      open = false;
      ondoubletap();
      return;
    }
    lastTapAt = tappedAt;
    open = !open;
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
    class={[buttonClass, ondoubletap && "touch-manipulation"]}
    aria-label={label}
    title={label}
    aria-expanded={open}
    onclick={toggleOrDoubleTap}
  >
    {@render button()}
  </Button>
  {#if open}
    <div class="header-menu-panel">
      {@render children(close)}
    </div>
  {/if}
</div>
