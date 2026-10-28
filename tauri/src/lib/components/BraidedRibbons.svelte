<!-- The parent must be positioned. -->
<script>
  import { onMount } from "svelte";
  import { mountBraidedRibbons } from "../waves/braided_ribbons.js";

  let stage;

  onMount(() => mountBraidedRibbons(stage).destroy);
</script>

<div class="braided-ribbons" bind:this={stage} aria-hidden="true"></div>

<style>
  .braided-ribbons {
    position: absolute;
    inset: calc(50% - 50px) 0 auto 0;
    height: 100px;
    z-index: -1;
    pointer-events: none;
  }
</style>
