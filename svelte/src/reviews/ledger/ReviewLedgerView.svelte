<!-- The Ledger view: the files of one group, each as a ledger with its code,
     under a line with the group's stats. -->
<script>
  import { msgid, ngettext } from "ttag";
  import LinesLabel from "../LinesLabel.svelte";
  import StaleIcon from "../StaleIcon.svelte";
  import { filesInGroup, groupLabel } from "../view_model.js";
  import LedgerFile from "./LedgerFile.svelte";

  /**
   * @type {{
   *   group: any,
   *   files: any[],
   *   row: any | undefined,
   *   outside: "dimmed" | "hidden",
   *   onJumpToFile: (fileId: string) => void,
   * }}
   */
  let { group, files, row, outside, onJumpToFile } = $props();

  let groupFiles = $derived(filesInGroup(files, group.id));

  /** Scrolls the page so the file's ledger is at the top, with a little room. */
  export function scrollToFile(fileId) {
    const element = document.getElementById(`review-file-${fileId}`);
    if (!element) return;
    element.scrollIntoView({ behavior: "smooth", block: "start" });
  }
</script>

<div class="flex shrink-0 flex-wrap items-center gap-x-4 gap-y-1 border-b border-base-300 px-6 py-2 text-[11px]">
  <span class="text-xs font-semibold">{groupLabel(group)}</span>
  {#if row}
    <span class="font-mono text-[11px] text-base-content/70">{ngettext(msgid`${row.stats.file_count} file`, `${row.stats.file_count} files`, row.stats.file_count)}</span>
    <LinesLabel added={row.stats.added_lines} removed={row.stats.removed_lines} />
    <StaleIcon count={row.stats.stale_caller_count} />
  {/if}
</div>
<div class="dot-grid grow">
  <div class="mx-auto w-max space-y-4 p-8">
    {#each groupFiles as file (file.id)}
      <LedgerFile {file} {files} {outside} {onJumpToFile} />
    {/each}
  </div>
</div>
