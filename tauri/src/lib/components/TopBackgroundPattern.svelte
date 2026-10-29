<!-- The app's background pattern: flow lines in the theme's gradient behind
     the top of the page, faded out before the content. -->
<script>
  import { PATTERNS } from "../background_patterns.js";
  import BackgroundPattern from "./BackgroundPattern.svelte";

  const HEIGHT = "150px";
  const SETTINGS = { size: 24, angle: 0, opacity: 0.22, fade: "top", color: "gradient" };

  const pattern = PATTERNS.find((each) => each.id === "flow-lines");
</script>

{#if pattern}
  <div class="top-background-pattern" style="height: {HEIGHT}">
    <BackgroundPattern {pattern} settings={SETTINGS} />
  </div>
{/if}

<style>
  /* Fixed, like the waves, so it stays put while the page scrolls. */
  .top-background-pattern {
    position: fixed;
    inset: 0 0 auto 0;
    z-index: -1;
  }
</style>
