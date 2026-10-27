<!-- The Ledger view: the files of one group, each as a ledger with its code.
     The tree on the left lists the groups and the files of the chosen one;
     clicking a file scrolls to it. -->
<script>
  import Icon from "../../lib/components/Icon.svelte";
  import { plural } from "../change_styles.js";
  import LinesLabel from "../LinesLabel.svelte";
  import StaleIcon from "../StaleIcon.svelte";
  import { UNGROUPED_ID, fileName, filesInGroup } from "../view_model.js";
  import LedgerFile from "./LedgerFile.svelte";

  /**
   * @type {{
   *   groups: any[],
   *   files: any[],
   *   rows: any[],
   *   groupId: string,
   *   onSelectGroup: (groupId: string) => void,
   *   onJumpToFile: (fileId: string) => void,
   * }}
   */
  let { groups, files, rows, groupId, onSelectGroup, onJumpToFile } = $props();

  let group = $derived(groups.find((candidate) => candidate.id === groupId) ?? groups[0]);
  let groupFiles = $derived(filesInGroup(files, group.id));
  let row = $derived(rows.find((candidate) => candidate.group.id === group.id));

  /** Scrolls the page so the file's ledger is at the top, with a little room. */
  export function scrollToFile(fileId) {
    const element = document.getElementById(`review-file-${fileId}`);
    if (!element) return;
    element.style.scrollMarginTop = "16px";
    element.scrollIntoView({ behavior: "smooth", block: "start" });
  }
</script>

<div class="flex items-start bg-base-100">
  <aside class="sticky top-0 max-h-[calc(100vh-7rem)] w-60 shrink-0 self-start overflow-y-auto border-r border-base-300 py-2">
    <div class="mb-1 px-3 pb-2 text-[10px] uppercase tracking-wide text-base-content/50">Groups</div>
    <nav>
      {#each groups as candidate (candidate.id)}
        <button
          type="button"
          onclick={() => onSelectGroup(candidate.id)}
          class={[
            "flex w-full items-center gap-1.5 py-1 pl-3 pr-3 text-left text-xs",
            candidate.id === group.id ? "bg-primary/10 font-semibold" : "hover:bg-base-200/70",
            candidate.id === UNGROUPED_ID ? "text-base-content/60" : "",
          ]}
        >
          <span class="shrink-0 text-base-content/50"><Icon name="hero-folder-mini" class="size-3.5" /></span>
          <span class="min-w-0 flex-1 truncate" title={candidate.label}>{candidate.label}</span>
          <span class="shrink-0 font-mono text-[10px] text-base-content/40">{filesInGroup(files, candidate.id).length}</span>
        </button>
        {#if candidate.id === group.id}
          {#each groupFiles as file (file.id)}
            <button
              type="button"
              onclick={() => scrollToFile(file.id)}
              title={file.path}
              class="flex w-full items-center gap-1.5 py-1 pl-7 pr-3 text-left font-mono text-[11px] hover:bg-base-200/70"
            >
              <span class="shrink-0 text-base-content/50"><Icon name="hero-document-text-micro" class="size-3.5" /></span>
              <span class="min-w-0 flex-1 truncate">{fileName(file.path)}</span>
            </button>
          {/each}
        {/if}
      {/each}
    </nav>
  </aside>

  <div class="min-w-0 flex-1">
    <div class="flex flex-wrap items-center gap-x-4 gap-y-1 border-b border-base-300 px-4 py-2 text-[11px]">
      <span class="text-xs font-semibold">{group.label}</span>
      {#if row}
        <span class="font-mono text-[11px] text-base-content/70">{plural(row.stats.file_count, "file")}</span>
        <LinesLabel added={row.stats.added_lines} removed={row.stats.removed_lines} />
        <StaleIcon count={row.stats.stale_caller_count} />
      {/if}
    </div>
    <div class="dot-grid overflow-x-auto">
      <div class="mx-auto w-max space-y-4 p-8">
        {#each groupFiles as file (file.id)}
          <LedgerFile {file} {files} {onJumpToFile} />
        {/each}
      </div>
    </div>
  </div>
</div>
