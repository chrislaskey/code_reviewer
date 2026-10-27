<!-- One line of a ledger column. Hovering it highlights the changed function
     it belongs to; lines of other functions fade. -->
<script>
  import MarkerIcon from "../MarkerIcon.svelte";

  /**
   * @type {{
   *   label: string,
   *   detail?: string,
   *   markers?: string[],
   *   stale?: boolean,
   *   dim?: boolean,
   *   faded?: boolean,
   *   highlighted?: boolean,
   *   height?: number,
   *   onhover: (hovering: boolean) => void,
   *   onclick?: () => void,
   *   children?: import("svelte").Snippet,
   * }}
   */
  let {
    label,
    detail,
    markers = [],
    stale = false,
    dim = false,
    faded = false,
    highlighted = false,
    height = 30,
    onhover,
    onclick,
    children,
  } = $props();
</script>

<svelte:element
  this={onclick ? "button" : "div"}
  type={onclick ? "button" : undefined}
  role={onclick ? undefined : "listitem"}
  {onclick}
  onmouseenter={() => onhover(true)}
  onmouseleave={() => onhover(false)}
  title={label}
  style="height: {height}px"
  class={[
    "flex w-full items-center gap-2 rounded-md px-2 text-left font-mono text-[11px]",
    onclick ? "cursor-pointer" : "cursor-default",
    highlighted ? "bg-base-content/10" : "hover:bg-base-content/5",
    faded ? "opacity-25" : "",
    dim ? "text-base-content/60" : "",
    stale ? "text-error" : "",
  ]}
>
  {#if children}
    {@render children()}
  {:else}
    <span class="min-w-0 flex-1 truncate">{label}</span>
  {/if}
  {#if detail}<span class="shrink-0 text-[10px] text-base-content/50">{detail}</span>{/if}
  {#each markers as marker (marker)}
    <MarkerIcon {marker} />
  {/each}
</svelte:element>
