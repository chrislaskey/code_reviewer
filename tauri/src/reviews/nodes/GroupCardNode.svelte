<!-- A group as a card: the group in the header, its files as the content.
     On the All changes row the content is the groups. A file line opens the
     Ledger view of its group; a group line scrolls the canvas to that group. -->
<script>
  import { getContext } from "svelte";
  import { Handle, Position } from "@xyflow/svelte";
  import Icon from "../../lib/components/Icon.svelte";
  import { CHANGE_STYLES, plural } from "../change_styles.js";
  import LinesLabel from "../LinesLabel.svelte";
  import StaleIcon from "../StaleIcon.svelte";
  import StartEllipsis from "../StartEllipsis.svelte";
  import { ALL_CHANGES_ID, UNGROUPED_ID } from "../view_model.js";
  import { CARD_PARTS, SIZES } from "./layout.js";

  let { data } = $props();
  const { focus, hoverLine, openFile, showGroup } = getContext("review-nodes");

  let row = $derived(data.row);
  let status = $derived(CHANGE_STYLES[row.status]);
  let isAllChanges = $derived(row.group.id === ALL_CHANGES_ID);
  let subtitle = $derived(
    isAllChanges ? "every group" : row.group.id === UNGROUPED_ID ? "files in no group" : row.group.id,
  );

  function lineTitle(line) {
    return line.kind === "group" ? `Show ${line.label}` : `Open the functions of ${line.label}`;
  }

  function openLine(line) {
    if (line.kind === "group") showGroup(line.id);
    else openFile(row.group.id, line.id);
  }
</script>

<div class="relative rounded-xl border border-base-300 bg-base-100 shadow-sm" style="width: {SIZES.cardWidth}px">
  <div class="flex items-center gap-3 px-4" style="height: {CARD_PARTS.header}px">
    <span class="relative inline-flex shrink-0">
      <span class="inline-flex size-10 items-center justify-center rounded-full bg-base-content text-base-100">
        <Icon name={isAllChanges ? "hero-rectangle-stack-micro" : "hero-folder-mini"} class="size-5" />
      </span>
      <span class={["absolute -bottom-0.5 -right-0.5 size-3.5 rounded-full ring-2 ring-base-100", status.dot]}></span>
    </span>
    <div class="min-w-0 flex-1">
      <div class="flex items-baseline justify-between gap-2">
        <h3 class="truncate text-sm font-semibold" title={row.group.label}>{row.group.label}</h3>
        <span class="font-mono text-[10px] text-base-content/50">{plural(row.stats.file_count, "file")}</span>
      </div>
      <p class="truncate font-mono text-[11px] text-base-content/60">Group · {subtitle}</p>
    </div>
  </div>

  <div class="border-t border-base-content/10" style="padding-bottom: {CARD_PARTS.bottom}px">
    <div class="flex items-center px-4 text-[10px] uppercase tracking-wide text-base-content/40" style="height: {CARD_PARTS.label}px">
      {row.content_label}
    </div>
    {#each row.lines as line (line.key)}
      {@const lineStatus = line.status ? CHANGE_STYLES[line.status] : null}
      {@const hasFunctions = line.kind === "group" || line.stats.function_count > 0}
      <button
        type="button"
        onclick={() => openLine(line)}
        onmouseenter={() => hoverLine(line.key)}
        onmouseleave={() => hoverLine(null)}
        title={lineTitle(line)}
        style="height: {CARD_PARTS.line}px"
        class={[
          "flex w-full cursor-pointer items-center gap-2 px-4 text-left font-mono text-xs",
          focus.key === line.key ? "bg-base-content/10" : "hover:bg-base-content/5",
        ]}
      >
        {#if lineStatus}
          <span class={["w-3 shrink-0 text-center font-bold", lineStatus.text]}>{lineStatus.sign}</span>
        {/if}
        {#if line.kind === "group"}
          <span class="min-w-0 flex-1 truncate font-semibold">{line.label}</span>
        {:else}
          <span class="min-w-0 flex-1"><StartEllipsis text={line.label} /></span>
        {/if}
        {#if hasFunctions}
          <span class="shrink-0 text-[10px] text-base-content/50">
            {line.kind === "group" ? plural(line.stats.file_count, "file") : plural(line.stats.function_count, "function")}
          </span>
          <LinesLabel added={line.stats.added_lines} removed={line.stats.removed_lines} />
        {/if}
        <StaleIcon count={line.stats.stale_caller_count} />
        <Icon name="hero-chevron-right-micro" class="size-3 shrink-0 text-base-content/30" />
      </button>
    {/each}
  </div>

  <Handle type="target" position={Position.Left} style="top: {SIZES.cardHandleTop}px" />
  <Handle type="source" position={Position.Right} style="top: {SIZES.cardHandleTop}px" />
</div>
