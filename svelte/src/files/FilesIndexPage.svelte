<!-- The files in the person's communities, newest first, at /files,
     found by name, as a list or a grid (files_view.js). The page holds
     only the pages of files it shows, 200 at a time, asked of the core
     as a search: the list shows the first and loads more with a button;
     the grid, FilesGrid, asks for those it scrolls to. In the list, each
     with a small picture when it is an image, its name, its size, its
     communities, and where it stands when it is not on the server yet.
     A click opens its page. At the bottom in the app, the copies kept on
     this device and a button that removes them. See
     docs/design/files-browsing.md. -->
<script>
  import AnimationsToggle from "../lib/AnimationsToggle.svelte";
  import FileImage from "../lib/FileImage.svelte";
  import FilesGrid from "./FilesGrid.svelte";
  import { filesCacheDelete, filesCacheGet, filesContentUrl, filesSearch, onSynced } from "../lib/core/index.js";
  import { filesNewUrl, filesShowUrl } from "../lib/routes.js";
  import { fileSize } from "./file_size.js";
  import { filesView, setFilesView } from "./files_view.js";
  import { Badge, Button, ErrorMessage, Heading, Icon, Input, Link, Panel, Text } from "../lib/components/index.js";
  import Page from "../lib/Page.svelte";

  const PAGE_SIZE = 200;
  const SEARCH_WAIT_MS = 200;

  /** @type {{ view: import("./files_view.js").FilesView, label: string, icon: string }[]} */
  const VIEW_CHOICES = [
    { view: "list", label: "List", icon: "hero-list-bullet-mini" },
    { view: "grid", label: "Grid", icon: "hero-squares-2x2-mini" },
  ];

  let name = $state("");
  /** @type {number | null} how many files match, null until the first answer */
  let total = $state(null);
  /** @type {any[]} the files by position, a hole where one has not come yet */
  let rows = $state.raw([]);
  let error = $state("");
  /** @type {{count: number, bytes: number} | null} */
  let cache = $state(null);
  let view = $state(filesView());

  /** The pages asked for since the search started over. */
  let pagesAsked = new Set();
  /** Counts the times the search started over, so an older answer is dropped. */
  let generation = 0;
  /** The positions on screen, which a sync fetches again. */
  let shown = { start: 0, end: PAGE_SIZE };
  /** @type {ReturnType<typeof setTimeout> | undefined} */
  let searchTimer;

  let listed = $derived(firstHole(rows));

  /** @param {any[]} files */
  function firstHole(files) {
    let count = 0;
    while (count < files.length && files[count]) count += 1;
    return files.slice(0, count);
  }

  /**
   * @param {any[]} files the files by position, changed in place
   * @param {number} page
   * @param {any[]} found
   */
  function place(files, page, found) {
    found.forEach((file, index) => (files[page * PAGE_SIZE + index] = file));
  }

  /** @param {number} page */
  function searchPage(page) {
    return filesSearch({ name, offset: page * PAGE_SIZE, limit: PAGE_SIZE });
  }

  /**
   * @param {number} start
   * @param {number} end
   */
  function pagesOf(start, end) {
    const pages = [];
    for (let page = Math.floor(start / PAGE_SIZE); page * PAGE_SIZE < Math.max(end, 1); page += 1) pages.push(page);
    return pages;
  }

  /**
   * Asks again for every page on screen, and shows the answers together,
   * so the files shown are replaced at once rather than emptied first.
   */
  async function startOver() {
    generation += 1;
    const asked = generation;
    const pages = pagesOf(shown.start, shown.end);
    pagesAsked = new Set(pages);
    try {
      const answers = await Promise.all(pages.map(searchPage));
      if (asked !== generation) return;
      const next = [];
      answers.forEach(({ files }, index) => place(next, pages[index], files));
      total = answers[answers.length - 1].total;
      rows = next;
      error = "";
    } catch (problem) {
      if (asked === generation) error = String(problem);
    }
  }

  /** @param {number} page */
  async function loadPage(page) {
    if (pagesAsked.has(page)) return;
    pagesAsked.add(page);
    const asked = generation;
    try {
      const { total: matching, files } = await searchPage(page);
      if (asked !== generation) return;
      const next = rows.slice();
      place(next, page, files);
      total = matching;
      rows = next;
    } catch (problem) {
      if (asked !== generation) return;
      pagesAsked.delete(page);
      error = String(problem);
    }
  }

  /**
   * @param {number} start
   * @param {number} end
   */
  function showPositions(start, end) {
    shown = { start, end };
    for (const page of pagesOf(start, end)) loadPage(page);
  }

  function loadMore() {
    showPositions(0, listed.length + PAGE_SIZE);
  }

  function searchAfterPause() {
    clearTimeout(searchTimer);
    searchTimer = setTimeout(() => {
      shown = { start: 0, end: PAGE_SIZE };
      startOver();
    }, SEARCH_WAIT_MS);
  }

  /** @param {import("./files_view.js").FilesView} chosen */
  function showAs(chosen) {
    view = chosen;
    setFilesView(chosen);
    if (chosen === "list") shown = { start: 0, end: Math.max(listed.length, PAGE_SIZE) };
  }

  async function loadCache() {
    try {
      cache = await filesCacheGet();
    } catch (problem) {
      error = String(problem);
    }
  }

  async function removeCached() {
    try {
      await filesCacheDelete();
      cache = await filesCacheGet();
    } catch (problem) {
      error = String(problem);
    }
  }

  $effect(() =>
    onSynced(() => {
      startOver();
      loadCache();
    }),
  );
  $effect(() => () => clearTimeout(searchTimer));
  startOver();
  loadCache();
</script>

<Page>
  <div class="flex items-center justify-between gap-4">
    <Heading level={1}>Files</Heading>
    <Button variant="primary" size="sm" href={filesNewUrl()}>New files</Button>
  </div>

  <ErrorMessage message={error} />

  {#if total === 0 && name === ""}
    <Text tone="muted">No files yet.</Text>
  {:else if total !== null}
    <Input type="search" bind:value={name} oninput={searchAfterPause} placeholder="Search by name" aria-label="Search by name" />
    <div class="flex flex-wrap items-center justify-between gap-3">
      <AnimationsToggle />
      <div class="join">
        {#each VIEW_CHOICES as choice (choice.view)}
          <Button
            size="sm"
            variant={view === choice.view ? "primary" : "default"}
            class="join-item"
            aria-pressed={view === choice.view}
            onclick={() => showAs(choice.view)}
          >
            <Icon name={choice.icon} />
            {choice.label}
          </Button>
        {/each}
      </div>
    </div>
    {#if total === 0}
      <Text tone="muted">No files have that in their name.</Text>
    {/if}
  {/if}

  {#if total && view === "grid"}
    <FilesGrid {total} {rows} onshow={showPositions} />
  {:else if total}
    <ul class="flex flex-col gap-3">
      {#each listed as { file, communities, shown_as, state, error: problem } (file.id)}
        <Panel as="li" layout="row">
          <a href={filesShowUrl(file.id)} class="flex size-12 shrink-0 items-center justify-center">
            {#if shown_as === "image"}
              <FileImage src={filesContentUrl(file.id)} alt={file.name} class="size-12 rounded object-cover" missing="" />
            {/if}
          </a>
          <div class="flex min-w-0 grow flex-col">
            <Link class="truncate" href={filesShowUrl(file.id)}>{file.name}</Link>
            <Text as="span" tone="muted">
              {fileSize(file.size)} · {file.inserted_at} · in {communities.map((community) => community.name).join(", ")}
            </Text>
            {#if problem}
              <Text as="span" tone="error" size="sm">{problem}</Text>
            {/if}
          </div>
          {#if state === "waiting"}
            <Badge>Waiting to upload</Badge>
          {:else if state === "refused"}
            <Badge tone="warning">Not saved on the server</Badge>
          {/if}
        </Panel>
      {/each}
    </ul>
    {#if listed.length < total}
      <Button class="self-center" onclick={loadMore}>Show more</Button>
    {/if}
  {/if}

  {#if cache}
    <div class="flex items-center justify-between gap-3">
      <Text tone="muted">
        {cache.count}
        {cache.count === 1 ? "file" : "files"} kept on this device, {fileSize(cache.bytes)}. Removing them frees the space;
        each is fetched again when it is next shown.
      </Text>
      <Button size="sm" class="shrink-0" disabled={cache.count === 0} onclick={removeCached}>Remove cached files</Button>
    </div>
  {/if}
</Page>
