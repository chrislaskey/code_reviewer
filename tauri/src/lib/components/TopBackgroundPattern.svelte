<!-- The app's background pattern: flow lines in the theme's gradient behind
     the top of the page, faded out before the content. -->
<script>
  import { flowLinesMaskImage } from "../flow_lines.js";

  const LINE_SIZE = 24;

  let width = $state(0);
  let height = $state(0);

  let maskImage = $derived(width === 0 ? "none" : flowLinesMaskImage(width, height, LINE_SIZE));
</script>

<div class="top-background-pattern" aria-hidden="true">
  <div class="faded-area" bind:clientWidth={width} bind:clientHeight={height}>
    <div class="flow-lines" style="mask-image: {maskImage};"></div>
  </div>
</div>

<style>
  /* Fixed, like the waves, so it stays put while the page scrolls; starts
     where the page does, clear of the gutter. */
  .top-background-pattern {
    position: fixed;
    inset: 0 0 auto var(--gutter-width);
    height: calc(150px + var(--title-bar-height));
    z-index: -1;
  }

  .faded-area {
    position: absolute;
    inset: 0;
    overflow: hidden;
    pointer-events: none;
    /* Faded out before the content, and in from the left, so the open tab
       in the gutter meets the lines softly. */
    mask-image:
      linear-gradient(to bottom, #000 0%, rgb(0 0 0 / 0.6) 35%, transparent 100%),
      linear-gradient(to right, transparent, #000 110px);
    mask-composite: intersect;
    -webkit-mask-composite: source-in;
  }

  .flow-lines {
    position: absolute;
    inset: 0;
    background: linear-gradient(110deg, var(--color-primary), var(--color-secondary), var(--color-accent));
    opacity: 0.22;
    mask-size: 100% 100%;
    mask-repeat: no-repeat;
  }
</style>
