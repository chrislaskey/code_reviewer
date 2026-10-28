<!-- The review page: runs the comparison in the URL, then shows the result
     in the Node view (groups and files on a canvas) or the Ledger view (the
     files of one group with their functions, calls, callers and code). The
     tree on the left lists All changes, the groups and the files of the
     chosen group, and one click always means the same view:
     - All changes shows the Node view, moved to the All changes row.
     - A group shows the Node view, moved to that group, and lists its files.
     - A file shows the Ledger view of its group, scrolled to the file. -->
<script>
  import { onMount, setContext, tick } from "svelte";
  import AppNav from "../lib/components/AppNav.svelte";
  import { analyzeFileChanges, getFileDiffs, getFunctionDiffs } from "../lib/backend.js";
  import { reviewNewUrl, reviewRequestFromSearch } from "../lib/routes.js";
  import { plural } from "../lib/change_styles.js";
  import { describeRequest } from "../lib/comparison_form.js";
  import MarkerIcon from "./MarkerIcon.svelte";
  import ReviewTree from "./ReviewTree.svelte";
  import SegmentedControl from "../lib/components/SegmentedControl.svelte";
  import Toolbar from "../lib/components/Toolbar.svelte";
  import OpenAllButtons from "./ledger/OpenAllButtons.svelte";
  import ReviewLedgerView from "./ledger/ReviewLedgerView.svelte";
  import { createOpenAllCommand } from "./ledger/open_all.svelte.js";
  import ReviewNodeView from "./nodes/ReviewNodeView.svelte";
  import { ALL_CHANGES_ID, findFileById, toViewFiles, toViewGroupRows } from "./view_model.js";

  const VIEW_OPTIONS = [
    { value: "nodes", label: "Nodes" },
    { value: "ledger", label: "Ledger" },
  ];

  const OUTSIDE_OPTIONS = [
    { value: "dimmed", label: "Dimmed" },
    { value: "hidden", label: "Hidden" },
  ];

  const request = reviewRequestFromSearch(window.location.search);

  let result = $state(null);
  let error = $state(null);
  let loading = $state(request !== null);
  let view = $state("nodes");
  /** The group whose files the tree lists, and the group of the Ledger view. */
  let selectedGroupId = $state(null);
  /** How calls into dependencies and the standard library are drawn. */
  let outside = $state(/** @type {"dimmed" | "hidden"} */ ("hidden"));
  let nodeView = $state();
  let ledgerView = $state();

  let files = $derived(result ? toViewFiles(result.files) : []);
  let groupRows = $derived(result ? toViewGroupRows(result.group_rows, files) : []);
  let allChangesRow = $derived(result ? toViewGroupRows([result.all_changes_row], files)[0] : null);
  let functionCount = $derived(files.reduce((sum, file) => sum + file.functions.length, 0));
  let selectedGroup = $derived(result?.groups.find((group) => group.id === selectedGroupId) ?? null);
  let selectedRow = $derived(groupRows.find((row) => row.group.id === selectedGroupId));

  const openAll = createOpenAllCommand();

  // The ledgers load their code through these, one request per file.
  setContext("review-code", {
    loadFunctionDiffs: (targets) => getFunctionDiffs({ ...comparison(), targets }),
    loadFileDiffs: (targets) => getFileDiffs({ ...comparison(), targets }),
  });

  function comparison() {
    return { repo_path: request.repo_path, base_commit: result.base_commit, head_commit: result.head_commit };
  }

  function shortCommit(commit) {
    return commit.slice(0, 8);
  }

  /** Shows the Node view moved to a group, with its files in the tree. */
  async function showGroupInNodes(groupId) {
    view = "nodes";
    selectedGroupId = groupId === ALL_CHANGES_ID ? null : groupId;
    await tick();
    nodeView?.showGroup(groupId);
  }

  /** Shows the Ledger view of a group, scrolled to one of its files. */
  async function openFileInLedger(groupId, fileId) {
    view = "ledger";
    selectedGroupId = groupId;
    await tick();
    ledgerView?.scrollToFile(fileId);
  }

  /** From a link in a ledger: the file in the open group, else in its first group. */
  function jumpToFile(fileId) {
    const file = findFileById(files, fileId);
    if (!file) return;
    const groupId = file.group_ids.includes(selectedGroupId) ? selectedGroupId : file.group_ids[0];
    openFileInLedger(groupId, fileId);
  }

  /** The Ledger view always has a group open; the first one when none was chosen. */
  function changeView(next) {
    view = next;
    if (next === "ledger" && selectedGroup === null) selectedGroupId = result.groups[0]?.id ?? null;
  }

  onMount(async () => {
    if (request === null) return;
    try {
      result = await analyzeFileChanges(request);
    } catch (caught) {
      error = String(caught);
    } finally {
      loading = false;
    }
  });
</script>

<div class="full-page">
  <AppNav current="reviews" />

  <div class="flex shrink-0 flex-wrap items-center gap-x-4 gap-y-1 border-b border-base-300 bg-base-100 px-4 py-2 font-mono text-[11px] text-base-content/70">
    {#if request}
      <span class="text-base-content">{request.repo_path}</span>
      <span>{describeRequest(request)}</span>
      {#if result}
        <span>
          {plural(result.files.length, "changed file")}, {plural(functionCount, "changed function")} ·
          {shortCommit(result.base_commit)} → {result.head_commit ? shortCommit(result.head_commit) : "working tree"}
        </span>
        {#if result.skipped_files.length > 0}
          <details class="relative">
            <summary class="cursor-pointer">{plural(result.skipped_files.length, "skipped file")}</summary>
            <ul class="absolute z-10 mt-1 max-h-64 overflow-auto rounded-lg border border-base-300 bg-base-100 p-2 shadow">
              {#each result.skipped_files as skipped (skipped.path)}
                <li>{skipped.path} · {skipped.reason}</li>
              {/each}
            </ul>
          </details>
        {/if}
      {/if}
    {/if}
    <a href={reviewNewUrl(request)} class="ml-auto text-primary">Edit review</a>
  </div>

  {#if request === null}
    <p class="px-6 py-10 text-sm text-base-content/50">
      This page needs a review to show. <a href={reviewNewUrl()}>Start a new review.</a>
    </p>
  {:else if loading}
    <p class="px-6 py-10 text-sm text-base-content/50">Analyzing the changes…</p>
  {:else if error}
    <p class="border-b border-base-300 bg-error/10 px-4 py-3 font-mono text-xs text-error">{error}</p>
  {:else if result.files.length === 0}
    <p class="px-6 py-10 text-sm text-base-content/50">Nothing changed between the two.</p>
  {:else}
    <Toolbar>
      <SegmentedControl label="View" value={view} options={VIEW_OPTIONS} onchange={changeView} />
      {#if view === "ledger"}
        <SegmentedControl
          label="Dependencies and standard library"
          value={outside}
          options={OUTSIDE_OPTIONS}
          onchange={(value) => (outside = value === "dimmed" ? "dimmed" : "hidden")}
        />
        <OpenAllButtons oncommand={openAll.send} />
      {/if}
      <span class="text-[11px] text-base-content/50">
        {#if view === "nodes"}
          Click a file to open its group in the Ledger view, a group to show it.
        {:else}
          Hover a function to see its code, click the code to keep it open.
        {/if}
      </span>
      <ul class="ml-auto flex flex-wrap gap-x-5 text-[11px] text-base-content/70">
        <li class="flex items-center gap-1.5"><MarkerIcon marker="stale" /> stale caller</li>
        <li class="flex items-center gap-1.5"><MarkerIcon marker="also_changed" /> also changed, click to jump</li>
      </ul>
    </Toolbar>
    <div class="full-page-body">
      <aside class="w-60 shrink-0 overflow-y-auto border-r border-base-300 bg-base-200">
        <ReviewTree
          {allChangesRow}
          groups={result.groups}
          {files}
          {selectedGroupId}
          onSelectAllChanges={() => showGroupInNodes(ALL_CHANGES_ID)}
          onSelectGroup={showGroupInNodes}
          onSelectFile={openFileInLedger}
        />
      </aside>
      {#if view === "nodes"}
        <div class="full-page-canvas">
          <ReviewNodeView
            bind:this={nodeView}
            rows={[allChangesRow, ...groupRows]}
            onOpenFile={openFileInLedger}
            onShowGroup={showGroupInNodes}
          />
        </div>
      {:else}
        <div class="full-page-content">
          <ReviewLedgerView
            bind:this={ledgerView}
            group={selectedGroup}
            {files}
            row={selectedRow}
            {outside}
            onJumpToFile={jumpToFile}
          />
        </div>
      {/if}
    </div>
  {/if}
</div>
