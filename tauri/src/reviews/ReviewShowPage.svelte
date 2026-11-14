<!-- The review page: runs the comparison in the URL, then shows the result
     in the Files tab, the Node view (groups and files on a canvas), or the
     Code tab, the Ledger view (the files of one group with their functions,
     calls, callers and code). The tree on the left lists All changes, the
     groups and the files of the chosen group, and one click always means the
     same view:
     - All changes shows the Node view, moved to the All changes row.
     - A group shows the Node view, moved to that group, and lists its files.
     - A file shows the Ledger view of its group, scrolled to the file.
     Small windows have one column: on Files the tree, full width, above the
     canvas, and on Code only the code. -->
<script>
  import { onMount, setContext, tick } from "svelte";
  import AppNav from "../lib/components/AppNav.svelte";
  import { analyzeFileChanges, getFileDiffs, getFunctionDiffs } from "../lib/backend.js";
  import { reviewNewUrl, reviewRequestFromSearch } from "../lib/routes.js";
  import { plural } from "../lib/change_styles.js";
  import { describeRequest } from "../lib/comparison_form.js";
  import ChoiceMenu from "../lib/components/ChoiceMenu.svelte";
  import Icon from "../lib/components/Icon.svelte";
  import InfoPopover from "../lib/components/InfoPopover.svelte";
  import MenuChoice from "../lib/components/MenuChoice.svelte";
  import MarkerIcon from "./MarkerIcon.svelte";
  import ReviewTree from "./ReviewTree.svelte";
  import SegmentedControl from "../lib/components/SegmentedControl.svelte";
  import Toolbar from "../lib/components/Toolbar.svelte";
  import TopBackgroundPattern from "../lib/components/TopBackgroundPattern.svelte";
  import OpenAllButtons from "./ledger/OpenAllButtons.svelte";
  import ReviewLedgerView from "./ledger/ReviewLedgerView.svelte";
  import { createOpenAllCommand } from "./ledger/open_all.svelte.js";
  import ReviewNodeView from "./nodes/ReviewNodeView.svelte";
  import { ALL_CHANGES_ID, findFileById, toViewFiles, toViewGroupRows } from "./view_model.js";

  const VIEW_OPTIONS = [
    { value: "nodes", label: "Files" },
    { value: "ledger", label: "Code" },
  ];

  // How the Code tab draws calls into dependencies and the standard library.
  const OUTSIDE_CHOICES = [
    { value: "hidden", label: "Your code", explanation: "Calls into dependencies and the standard library are hidden." },
    { value: "dimmed", label: "All calls", explanation: "Calls into dependencies and the standard library are shown, dimmed." },
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
  let outsideLabel = $derived(OUTSIDE_CHOICES.find((choice) => choice.value === outside)?.label);

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

<TopBackgroundPattern />

<AppNav current="reviews" />

<div class="full-page max-sm:h-auto">
  <div class="full-page-title">
    <h1>Review</h1>
    {#if request}
      <span class="comparison font-mono text-[11px] text-base-content/70">
        <span class="text-base-content">{request.repo_path}</span>
        <span>{describeRequest(request)}</span>
      </span>
    {/if}
    <a href={reviewNewUrl(request)} class="btn btn-ghost btn-sm ml-auto">
      <Icon name="hero-pencil-square-micro" class="size-4" />
      Edit
    </a>
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
    <!-- On small windows the page scrolls as a whole, and the toolbar stays at the top. -->
    <Toolbar class="review-toolbar max-sm:sticky max-sm:top-0 max-sm:z-10 max-sm:bg-base-100">
      <SegmentedControl value={view} options={VIEW_OPTIONS} onchange={changeView} />
      {#if view === "ledger"}
        <div class="flex items-center gap-1">
          <ChoiceMenu title="Which calls the code shows" class="btn btn-ghost btn-sm" align="start">
            {#snippet buttonContent()}
              <span class="max-sm:hidden">Show</span>
              {outsideLabel}
            {/snippet}
            {#snippet children({ close })}
              {#each OUTSIDE_CHOICES as choice (choice.value)}
                <MenuChoice
                  label={choice.label}
                  explanation={choice.explanation}
                  onclick={() => {
                    outside = choice.value === "dimmed" ? "dimmed" : "hidden";
                    close();
                  }}
                />
              {/each}
            {/snippet}
          </ChoiceMenu>
          <OpenAllButtons oncommand={openAll.send} />
        </div>
      {/if}
      <div class="ml-auto flex items-center gap-3">
        <span class="font-mono text-[11px] text-base-content/50 max-sm:hidden">
          {plural(result.files.length, "file")} · {plural(functionCount, "function")}
        </span>
        <InfoPopover title="How to use this view, what its markers mean, and what was compared" class="dropdown-end">
          <p>
            {#if view === "nodes"}
              Click a group to show it, a file to open its group's code.
            {:else}
              Hover a function to see its code, click the code to keep it open.
            {/if}
          </p>
          <ul class="mt-2 flex flex-col gap-1">
            <li class="flex items-center gap-1.5"><MarkerIcon marker="stale" /> stale caller</li>
            <li class="flex items-center gap-1.5"><MarkerIcon marker="also_changed" /> also changed, click to jump</li>
          </ul>
          <p class="mt-2 font-mono text-xs">
            {plural(result.files.length, "changed file")}, {plural(functionCount, "changed function")} ·
            {shortCommit(result.base_commit)} → {result.head_commit ? shortCommit(result.head_commit) : "working tree"}
          </p>
          {#if result.skipped_files.length > 0}
            <p class="mt-2">{plural(result.skipped_files.length, "skipped file")}:</p>
            <ul class="max-h-40 overflow-auto font-mono text-xs">
              {#each result.skipped_files as skipped (skipped.path)}
                <li>{skipped.path} · {skipped.reason}</li>
              {/each}
            </ul>
          {/if}
        </InfoPopover>
      </div>
    </Toolbar>
    <div class="full-page-body max-sm:flex-col">
      <aside
        class={[
          "w-60 shrink-0 overflow-y-auto border-r border-base-300",
          "max-sm:w-full max-sm:overflow-visible max-sm:border-r-0",
          view === "ledger" && "max-sm:hidden",
        ]}
      >
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
        <div class="full-page-canvas max-sm:h-[60svh] max-sm:flex-none max-sm:border-t max-sm:border-base-300">
          <ReviewNodeView
            bind:this={nodeView}
            rows={[allChangesRow, ...groupRows]}
            onOpenFile={openFileInLedger}
            onShowGroup={showGroupInNodes}
          />
        </div>
      {:else}
        <div class="full-page-content flex flex-col max-sm:overflow-y-visible">
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
