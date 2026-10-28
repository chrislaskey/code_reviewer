<!-- The tree beside a review, like the file list of a pull request: All changes
     at the top, every group under it, and the files of the chosen group under
     that group. What a click does is up to the page. -->
<script>
  import Icon from "../lib/components/Icon.svelte";
  import { ALL_CHANGES_ID, UNGROUPED_ID, fileName, filesInGroup } from "./view_model.js";

  /**
   * @type {{
   *   allChangesRow: any,
   *   groups: any[],
   *   files: any[],
   *   selectedGroupId: string | null,
   *   onSelectAllChanges: () => void,
   *   onSelectGroup: (groupId: string) => void,
   *   onSelectFile: (groupId: string, fileId: string) => void,
   * }}
   */
  let { allChangesRow, groups, files, selectedGroupId, onSelectAllChanges, onSelectGroup, onSelectFile } = $props();

  const INDENT = 14;

  let items = $derived([
    {
      id: ALL_CHANGES_ID,
      label: allChangesRow.group.label,
      depth: 0,
      icon: "hero-rectangle-stack-micro",
      detail: String(allChangesRow.stats.file_count),
      onclick: onSelectAllChanges,
    },
    ...groups.flatMap((group) => {
      const item = {
        id: group.id,
        label: group.label,
        depth: 1,
        icon: "hero-folder-mini",
        detail: String(filesInGroup(files, group.id).length),
        dim: group.id === UNGROUPED_ID,
        selected: group.id === selectedGroupId,
        onclick: () => onSelectGroup(group.id),
      };
      if (group.id !== selectedGroupId) return [item];
      const fileItems = filesInGroup(files, group.id).map((file) => ({
        id: `${group.id}:${file.id}`,
        label: fileName(file.path),
        title: file.path,
        depth: 2,
        icon: "hero-document-text-micro",
        mono: true,
        onclick: () => onSelectFile(group.id, file.id),
      }));
      return [item, ...fileItems];
    }),
  ]);
</script>

<nav class="py-2">
  <div class="mb-1 px-3 pb-2 text-[10px] uppercase tracking-wide text-base-content/50">Groups</div>
  {#each items as item (item.id)}
    <button
      type="button"
      onclick={item.onclick}
      title={item.title ?? item.label}
      style="padding-left: {12 + item.depth * INDENT}px"
      class={[
        "flex w-full items-center gap-1.5 py-1 pr-3 text-left",
        item.selected ? "bg-primary/10 font-semibold" : "hover:bg-base-content/5",
        item.dim ? "text-base-content/60" : "",
        item.mono ? "font-mono text-[11px]" : "text-xs",
      ]}
    >
      <span class="shrink-0 text-base-content/50"><Icon name={item.icon} class="size-3.5" /></span>
      <span class="min-w-0 flex-1 truncate">{item.label}</span>
      {#if item.detail}<span class="shrink-0 font-mono text-[10px] text-base-content/40">{item.detail}</span>{/if}
    </button>
  {/each}
</nav>
