<!-- One changed file as a ledger: the header, then three columns. File lists
     the changed functions, Outputs the calls they make into the project,
     Inputs the callers in other modules. The code is under the columns.
     Hover any line to highlight the function it belongs to; the rest fades. -->
<script>
  import Icon from "../../lib/components/Icon.svelte";
  import { CHANGE_STYLES, plural } from "../change_styles.js";
  import LinesLabel from "../LinesLabel.svelte";
  import { findFileByPath, shortPath } from "../view_model.js";
  import CodeFooter from "./CodeFooter.svelte";
  import FileCodeFooter from "./FileCodeFooter.svelte";
  import LedgerLine from "./LedgerLine.svelte";

  /** @type {{ file: any, files: any[], onJumpToFile: (fileId: string) => void }} */
  let { file, files, onJumpToFile } = $props();

  const WIDTH = 1040;
  const COLUMNS = "270px 348px 348px";
  const LINE = 30;
  const STACKED_LINE = 40;

  let expanded = $state(true);
  let hovered = $state(null);

  let status = $derived(CHANGE_STYLES[file.status]);

  let outputs = $derived(
    file.callees
      .filter((callee) => callee.highlighted)
      .map((callee, index) => ({
        id: `out:${callee.label}:${callee.from}:${index}`,
        label: callee.label,
        detail: callee.call_count > 1 ? `×${callee.call_count}` : callee.from,
        markers: callee.also_changed ? ["also_changed"] : [],
        hoverKey: callee.from,
        jumpFileId: callee.also_changed ? (findFileByPath(files, callee.file_path)?.id ?? null) : null,
      })),
  );
  let foldedOutputCount = $derived(file.callees.filter((callee) => !callee.highlighted).length);

  let inputs = $derived(
    file.callers.map((caller, index) => ({
      id: `in:${caller.label}:${caller.line}:${index}`,
      label: caller.label,
      detail: caller.calls,
      subdetail: `${shortPath(caller.file_path)}:${caller.line}`,
      markers: [...(caller.stale ? ["stale"] : []), ...(caller.also_changed ? ["also_changed"] : [])],
      stale: caller.stale,
      hoverKey: caller.calls,
      jumpFileId: caller.also_changed ? (findFileByPath(files, caller.file_path)?.id ?? null) : null,
    })),
  );

  function stateOf(hoverKey) {
    return {
      faded: hovered !== null && hovered !== hoverKey,
      highlighted: hovered !== null && hovered === hoverKey,
    };
  }

  function hover(hoverKey) {
    return (hovering) => (hovered = hovering ? hoverKey : null);
  }

  function jumpTo(entry) {
    return entry.jumpFileId ? () => onJumpToFile(entry.jumpFileId) : undefined;
  }
</script>

<div id="review-file-{file.id}" class="overflow-hidden rounded-xl border border-base-300 bg-base-100 shadow-sm" style="width: {WIDTH}px">
  <button
    type="button"
    onclick={() => (expanded = !expanded)}
    class="flex h-[52px] w-full cursor-pointer items-center gap-3 px-4 text-left"
  >
    <Icon name={expanded ? "hero-chevron-down-micro" : "hero-chevron-right-micro"} class="size-4 text-base-content/50" />
    <span class={["size-2.5 shrink-0 rounded-full", status.dot]}></span>
    <h3 class="text-sm font-semibold">{file.module}</h3>
    <span class="min-w-0 truncate font-mono text-[11px] text-base-content/50">{file.path}</span>
    <span class="ml-auto shrink-0 font-mono text-[11px] text-base-content/60">
      {#if !file.plain}{plural(file.functions.length, "function")}{/if}
    </span>
  </button>

  {#if expanded && !file.plain}
    <div class="grid border-t border-base-content/10 p-3" style="grid-template-columns: {COLUMNS}; column-gap: 24px">
      <div>
        <div class="h-5 text-[10px] font-semibold uppercase tracking-wide text-base-content/40">File</div>
        {#each file.functions as fn (fn.key)}
          {@const fnStyle = CHANGE_STYLES[fn.kind]}
          <LedgerLine label={fnStyle.label} onhover={hover(fn.key)} {...stateOf(fn.key)}>
            <span class={["w-3 text-center font-bold", fnStyle.text]}>{fnStyle.sign}</span>
            <span class="min-w-0 flex-1 truncate">
              {fn.key}
              {#if fn.previous_key}<span class="text-base-content/40">← {fn.previous_key}</span>{/if}
            </span>
            {#if fn.added_lines !== null || fn.removed_lines !== null}
              <LinesLabel added={fn.added_lines ?? 0} removed={fn.removed_lines ?? 0} class="text-[10px]" />
            {/if}
          </LedgerLine>
        {/each}
      </div>
      <div>
        <div class="h-5 text-[10px] font-semibold uppercase tracking-wide text-base-content/40">Outputs</div>
        {#each outputs as entry (entry.id)}
          <LedgerLine
            label={entry.label}
            detail={entry.detail}
            markers={entry.markers}
            height={LINE}
            onhover={hover(entry.hoverKey)}
            onclick={jumpTo(entry)}
            {...stateOf(entry.hoverKey)}
          />
        {:else}
          <div class="flex items-center px-2 font-mono text-[10px] text-base-content/40" style="height: {LINE}px">none</div>
        {/each}
        {#if foldedOutputCount > 0}
          <div class="flex items-center px-2 font-mono text-[10px] text-base-content/40" style="height: {LINE}px">
            +{foldedOutputCount} in dependencies and stdlib
          </div>
        {/if}
      </div>
      <div>
        <div class="h-5 text-[10px] font-semibold uppercase tracking-wide text-base-content/40">Inputs</div>
        {#each inputs as entry (entry.id)}
          <LedgerLine
            label={entry.label}
            detail={entry.detail}
            markers={entry.markers}
            stale={entry.stale}
            height={STACKED_LINE}
            onhover={hover(entry.hoverKey)}
            onclick={jumpTo(entry)}
            {...stateOf(entry.hoverKey)}
          >
            <span class="min-w-0 flex-1 leading-4">
              <span class="block truncate">{entry.label}</span>
              <span class="block truncate text-[10px] text-base-content/50">{entry.subdetail}</span>
            </span>
          </LedgerLine>
        {:else}
          <div class="flex items-center px-2 font-mono text-[10px] text-base-content/40" style="height: {LINE}px">
            none outside this file
          </div>
        {/each}
      </div>
    </div>
  {/if}

  {#if expanded}
    {#if file.plain}
      <FileCodeFooter {file} />
    {:else}
      <CodeFooter functions={file.functions} hoveredKey={hovered} />
    {/if}
  {/if}
</div>
