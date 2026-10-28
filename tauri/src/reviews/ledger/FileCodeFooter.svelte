<!-- The code under the ledger of a file that was not parsed: its changed
     lines with a few lines of context. Closed at first. A Markdown file shows
     the new version of each changed stretch as HTML, or the raw lines. -->
<script>
  import { getContext } from "svelte";
  import Icon from "../../lib/components/Icon.svelte";
  import SegmentedControl from "../../lib/components/SegmentedControl.svelte";
  import CodeLines from "../../lib/components/CodeLines.svelte";
  import LineCounts from "./LineCounts.svelte";
  import { followOpenAllCommand } from "./open_all.svelte.js";
  import { languageForPath } from "../../lib/syntax_highlight.js";

  /** @type {{ file: any }} */
  let { file } = $props();

  const loaders = getContext("review-code");

  const MARKDOWN_VIEW_OPTIONS = [
    { value: "html", label: "HTML" },
    { value: "raw", label: "Raw" },
  ];

  /** @typedef {{ state: "loading" } | { state: "failed", message: string } | { state: "ready", diff: any }} Loaded */
  let loaded = $state(/** @type {Loaded} */ ({ state: "loading" }));
  $effect(() => {
    const target = file.file_diff_target;
    let outdated = false;
    loaded = { state: "loading" };
    loaders
      .loadFileDiffs([target])
      .then((items) => {
        if (!outdated) loaded = { state: "ready", diff: items[0] };
      })
      .catch((error) => {
        if (!outdated) loaded = { state: "failed", message: String(error) };
      });
    return () => {
      outdated = true;
    };
  });

  const opened = followOpenAllCommand(false);
  let markdownView = $state("html");
  let language = $derived(languageForPath(file.path));
  let diff = $derived(loaded.state === "ready" ? loaded.diff : null);
  let canShowHtml = $derived(diff !== null && diff.is_markdown && !diff.is_binary);
  let showHtml = $derived(canShowHtml && markdownView === "html");

  // The rendered Markdown may hold links. The app has no page to go to, so a
  // click does nothing.
  function keepLinksFromNavigating(event) {
    if (event.target.closest("a")) event.preventDefault();
  }
</script>

<div class="border-t border-base-content/10">
  <div class="flex h-5 items-center px-3 pt-2 text-[10px] uppercase tracking-wide text-base-content/40">Code</div>
  {#if loaded.state === "loading"}
    <p class="px-3 py-2 font-mono text-[11px] text-base-content/50">Loading code…</p>
  {:else if loaded.state === "failed"}
    <p class="px-3 py-2 font-mono text-[11px] text-error">{loaded.message}</p>
  {:else if diff}
    <button
      type="button"
      onclick={() => (opened.open = !opened.open)}
      class="flex h-8 w-full cursor-pointer items-center gap-2 px-3 text-left font-mono text-xs"
    >
      <Icon name="hero-chevron-right-micro" class={["size-3.5 text-base-content/50 transition-transform", opened.open ? "rotate-90" : ""]} />
      <span class="text-base-content/70">{diff.is_binary ? "Binary file" : "Changed lines"}</span>
      <LineCounts lines={diff.lines} />
    </button>
    {#if opened.open}
      {#if canShowHtml}
        <div class="flex justify-end px-3 pb-2">
          <SegmentedControl value={markdownView} options={MARKDOWN_VIEW_OPTIONS} onchange={(value) => (markdownView = value)} />
        </div>
      {/if}
      {#if diff.is_binary}
        <p class="px-3 pb-2 font-mono text-[11px] text-base-content/50">The contents of a binary file are not shown.</p>
      {:else if showHtml}
        {#if diff.html_hunks.length === 0}
          <p class="px-3 pb-2 font-mono text-[11px] text-base-content/50">
            Nothing is left of this text in the new version. Choose Raw to see the removed lines.
          </p>
        {:else}
          <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
          <div class="px-3 pb-2" onclick={keepLinksFromNavigating}>
            {#each diff.html_hunks as html, index (index)}
              {#if index > 0}<div class="my-2 select-none text-base-content/35">⋯</div>{/if}
              <div class="markdown-html">{@html html}</div>
            {/each}
          </div>
        {/if}
      {:else}
        <CodeLines lines={diff.lines} {language} />
      {/if}
      {#if diff.omitted_line_count > 0}
        <p class="px-3 pb-2 font-mono text-[11px] text-base-content/50">{diff.omitted_line_count} more lines are not shown.</p>
      {/if}
    {/if}
  {/if}
</div>
