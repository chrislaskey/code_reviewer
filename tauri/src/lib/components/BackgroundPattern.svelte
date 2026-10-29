<!-- One background pattern filling its positioned parent: the pattern turned
     by `angle`, faded out by `fade`, in `color`. Tiled patterns turn a square
     big enough to cover the area; generated ones draw for the area itself. -->
<script>
  import { COLORS, FADES, TONE_FILLS } from "../background_patterns.js";

  /**
   * @typedef {Object} Props
   * @property {import("../background_patterns.js").Pattern} pattern
   * @property {{ size: number, angle: number, opacity: number, fade: string, color: string }} settings
   */

  /** @type {Props} */
  let { pattern, settings } = $props();

  let width = $state(0);
  let height = $state(0);

  let side = $derived(Math.ceil(Math.hypot(width, height)));
  let fade = $derived(FADES.find((option) => option.value === settings.fade) ?? FADES[0]);
  let inkFill = $derived(COLORS.find((option) => option.value === (pattern.color ?? settings.color))?.fill ?? COLORS[0].fill);
  let layers = $derived(
    width === 0
      ? []
      : pattern.layers({ size: settings.size, angle: settings.angle, width: pattern.generated ? width : side, height: pattern.generated ? height : side }),
  );

  function layerStyle(layer) {
    const fill = layer.tone ? TONE_FILLS[layer.tone] : inkFill;
    const box = pattern.generated
      ? "inset: 0;"
      : `left: 50%; top: 50%; width: ${side}px; height: ${side}px; transform: translate(-50%, -50%) rotate(${settings.angle}deg);`;
    return [
      box,
      `background: ${fill};`,
      `opacity: ${settings.opacity};`,
      `mask-image: ${layer.image};`,
      `mask-size: ${layer.size ?? "100% 100%"};`,
      `mask-position: ${layer.position ?? "0 0"};`,
      `mask-repeat: ${pattern.generated ? "no-repeat" : "repeat"};`,
      `--tile-size: ${settings.size}px;`,
    ].join(" ");
  }
</script>

<div
  class="pattern-area"
  bind:clientWidth={width}
  bind:clientHeight={height}
  style="mask-image: {fade.mask}; mask-composite: {fade.composite ?? 'add'};"
  aria-hidden="true"
>
  {#each layers as layer, index (index)}
    <div class={["pattern-layer", pattern.animated && "drifting"]} style={layerStyle(layer)}></div>
  {/each}
</div>

<style>
  .pattern-area {
    position: absolute;
    inset: 0;
    overflow: hidden;
    pointer-events: none;
  }

  .pattern-layer {
    position: absolute;
  }

  .drifting {
    animation: drift 8s linear infinite;
  }

  @keyframes drift {
    to {
      mask-position: var(--tile-size) var(--tile-size);
    }
  }

  @media (prefers-reduced-motion: reduce) {
    .drifting {
      animation: none;
    }
  }
</style>
