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
  import FilesGrid from "./components/FilesGrid.svelte";
  import { filesCacheDelete, filesCacheGet, filesContentUrl, filesSearch, filesSearchWaitMs, onSynced } from "../lib/core/index.js";
  import { errorMessageOf } from "../lib/error_message.js";
  import { filesNewUrl, filesShowUrl } from "../lib/routes.js";
  import { fileSize } from "../lib/file_size.js";
  import { filesView, setFilesView } from "./files_view.js";
  import { Badge, Button, ErrorMessage, Heading, Icon, Input, Link, Page, Panel, Text } from "../lib/components/index.js";
  import { PAGE_SIZE, PagedSearch } from "../lib/paged_search.svelte.js";

  /** @type {{ view: import("./files_view.js").FilesView, label: string, icon: string }[]} */
  const VIEW_CHOICES = [
    { view: "list", label: "List", icon: "hero-list-bullet-mini" },
    { view: "grid", label: "Grid", icon: "hero-squares-2x2-mini" },
  ];

  let name = $state("");
  /** @type {{count: number, bytes: number} | null} */
  let cache = $state(null);
  let cacheError = $state("");
  let view = $state(filesView());

  const search = new PagedSearch((offset, limit) => filesSearch({ name, offset, limit }));
  /** @type {ReturnType<typeof setTimeout> | undefined} */
  let searchTimer;

  let total = $derived(search.total);
  let listed = $derived(firstHole(search.rows));
  let error = $derived(search.error || cacheError);

  /** @param {any[]} files */
  function firstHole(files) {
    let count = 0;
    while (count < files.length && files[count]) count += 1;
    return files.slice(0, count);
  }

  function loadMore() {
    search.showPositions(0, listed.length + PAGE_SIZE);
  }

  function searchAfterPause() {
    clearTimeout(searchTimer);
    searchTimer = setTimeout(() => search.startOverFromTop(), filesSearchWaitMs);
  }

  /** @param {import("./files_view.js").FilesView} chosen */
  function showAs(chosen) {
    view = chosen;
    setFilesView(chosen);
    if (chosen === "list") search.showPositions(0, Math.max(listed.length, PAGE_SIZE));
  }

  async function loadCache() {
    try {
      cache = await filesCacheGet();
      cacheError = "";
    } catch (problem) {
      cacheError = errorMessageOf(problem);
    }
  }

  async function removeCached() {
    try {
      await filesCacheDelete();
      cache = await filesCacheGet();
      cacheError = "";
    } catch (problem) {
      cacheError = errorMessageOf(problem);
    }
  }

  $effect(() =>
    onSynced(() => {
      search.startOver();
      loadCache();
    }),
  );
  $effect(() => () => clearTimeout(searchTimer));
  search.startOver();
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
    <FilesGrid {total} rows={search.rows} onshow={(start, end) => search.showPositions(start, end)} />
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
