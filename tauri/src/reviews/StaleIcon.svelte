<!-- A warning sign when callers still call something removed or renamed. -->
<script>
  import Icon from "../lib/components/Icon.svelte";
  import { plural } from "./change_styles.js";

  /** @type {{ count: number }} */
  let { count } = $props();
</script>

{#if count > 0}
  <span
    title="{plural(count, 'caller')} still calling a removed or renamed function"
    class="inline-flex shrink-0 text-error"
  >
    <Icon name="hero-exclamation-triangle-micro" class="size-3.5" />
  </span>
{/if}
