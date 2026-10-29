<script>
  import { onMount } from "svelte";
  import { mountStillWaves } from "../waves/still_waves.js";

  /** @type {{ rippleOnLoad?: boolean }} */
  let { rippleOnLoad = false } = $props();

  let stage;
  let waves;

  /**
   * @param {number} strength
   * @param {number} [x]
   */
  export function ripple(strength, x = stage.clientWidth * (0.15 + Math.random() * 0.7)) {
    waves?.ripple(x, strength);
  }

  // The waves are behind the page, so clicks on them land on the window.
  function rippleWhereClicked(event) {
    if (event.clientY < stage.getBoundingClientRect().top) return;
    if (event.target.closest("a, button, summary, input, select, textarea, label")) return;
    ripple(45, event.clientX);
  }

  onMount(() => {
    waves = mountStillWaves(stage);
    if (rippleOnLoad) ripple(35);
    return waves.destroy;
  });
</script>

<svelte:window onclick={rippleWhereClicked} />

<div class="still-waves" bind:this={stage} aria-hidden="true"></div>

<style>
  /* A background, so WebKit repaints it on a theme change. Starts where
     the page does, clear of the gutter. */
  .still-waves {
    background: var(--root-bg);
    position: fixed;
    inset: auto 0 0 var(--gutter-width);
    height: 220px;
    z-index: -1;
    pointer-events: none;
  }
</style>
