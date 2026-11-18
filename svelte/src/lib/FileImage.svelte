<!-- A file that is an image, wherever one is embedded: an <img>, so an
     animated GIF plays, or, with animations off (animations.js), its
     first frame drawn on a canvas, since a browser has no way to pause
     an <img>. An image that cannot be had, a file not on this device
     with the network off, shows `missing` instead. -->
<script>
  import { animationsOn, onAnimationsChanged } from "./animations.js";
  import { Text } from "./components/index.js";

  /**
   * @typedef {Object} Props
   * @property {string} src
   * @property {string} alt
   * @property {string} [class]
   * @property {string} [missing]
   */

  /** @type {Props} */
  let { src, alt, class: className = "", missing = "Not on this device" } = $props();

  let moving = $state(animationsOn());
  let failed = $state(false);
  /** @type {HTMLCanvasElement | undefined} */
  let canvas = $state();

  $effect(() => onAnimationsChanged((on) => (moving = on)));

  $effect(() => {
    if (moving || !canvas) return;
    const still = canvas;
    const image = new Image();
    image.onload = () => {
      still.width = image.naturalWidth;
      still.height = image.naturalHeight;
      still.getContext("2d")?.drawImage(image, 0, 0);
    };
    image.onerror = () => (failed = true);
    image.src = src;
  });
</script>

{#if failed}
  <Text as="span" tone="muted">{missing}</Text>
{:else if moving}
  <img {src} {alt} class={className} onerror={() => (failed = true)} />
{:else}
  <canvas bind:this={canvas} class={className}>{alt}</canvas>
{/if}
