<!-- A warning sign when callers still call something removed or renamed. -->
<script>
  import { msgid, ngettext } from "ttag";
  import Icon from "../lib/components/Icon.svelte";

  /** @type {{ count: number }} */
  let { count } = $props();
</script>

{#if count > 0}
  <span
    title={ngettext(
      msgid`${count} caller still calling a removed or renamed function`,
      `${count} callers still calling a removed or renamed function`,
      count,
    )}
    class="inline-flex shrink-0 text-error"
  >
    <Icon name="hero-exclamation-triangle-micro" class="size-3.5" />
  </span>
{/if}
