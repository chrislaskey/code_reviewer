<!-- A file outside the group on the other end of a call. When that file is
     itself changed in this review, a click scrolls to its group. -->
<script>
  import { getContext } from "svelte";
  import { Handle, Position } from "@xyflow/svelte";
  import { TONE_STYLES } from "../change_styles.js";
  import MarkerIcon from "../MarkerIcon.svelte";
  import StartEllipsis from "../StartEllipsis.svelte";
  import { SIZES, linkTone } from "./layout.js";

  let { data } = $props();
  const { focus, hoverLine, showGroup } = getContext("review-nodes");

  let link = $derived(data.link);
  let dimmed = $derived(focus.key !== null && focus.key !== link.line_key);
  let isInput = $derived(link.side === "input");
  let markers = $derived([...(link.is_stale ? ["stale"] : []), ...(link.file_id ? ["also_changed"] : [])]);
  let title = $derived(
    `${link.module}\n${link.group_label} · ${isInput ? "calls" : "called from"} ${link.line_label}${link.file_id ? "\nClick to show its group" : ""}`,
  );
</script>

<svelte:element
  this={link.file_id ? "button" : "div"}
  type={link.file_id ? "button" : undefined}
  role={link.file_id ? undefined : "listitem"}
  onclick={link.file_id ? () => showGroup(link.group_id) : undefined}
  onmouseenter={() => hoverLine(link.line_key)}
  onmouseleave={() => hoverLine(null)}
  {title}
  style="width: {SIZES.chipWidth}px; height: {SIZES.chipHeight}px; opacity: {dimmed ? 0.2 : 1}"
  class={[
    "relative flex items-center gap-2 rounded-lg border px-2.5 text-left shadow-sm transition-opacity",
    link.file_id ? "cursor-pointer hover:border-base-content/50" : "cursor-default",
    TONE_STYLES[linkTone(link)],
  ]}
>
  <div class="min-w-0 flex-1">
    <div class="truncate font-mono text-[11px] font-medium">{link.module}</div>
    <StartEllipsis text={link.path} class="font-mono text-[10px] text-base-content/50" />
  </div>
  {#if link.call_count > 1}
    <span class="rounded-full bg-base-content/10 px-1.5 font-mono text-[10px] font-semibold">×{link.call_count}</span>
  {/if}
  {#each markers as marker (marker)}
    <MarkerIcon {marker} />
  {/each}
  <Handle
    type={isInput ? "source" : "target"}
    position={isInput ? Position.Right : Position.Left}
    style="top: {SIZES.chipHandleTop}px"
  />
</svelte:element>
