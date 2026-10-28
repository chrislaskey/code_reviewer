<!-- A small icon after a caller or call, see MARKERS in change_styles.js. -->
<script>
  import Icon from "../lib/components/Icon.svelte";
  import { MARKERS } from "../lib/change_styles.js";

  /** @type {{ marker: string }} */
  let { marker } = $props();

  let style = $derived(MARKERS[marker]);
</script>

<span title={style.label} class={["inline-flex shrink-0", style.className]}>
  <Icon name={style.icon} class="size-3.5" />
</span>
