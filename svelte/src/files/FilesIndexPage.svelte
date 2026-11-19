<!-- The files in the person's communities, newest first, at /files:
     each with a small picture when it is an image, its name, its size,
     its communities, and where it stands when it is not on the server
     yet. A click opens its page. At
     the bottom in the app, the copies kept on this device and a button
     that removes them. -->
<script>
  import AnimationsToggle from "../lib/AnimationsToggle.svelte";
  import FileImage from "../lib/FileImage.svelte";
  import { filesCacheDelete, filesCacheGet, filesContentUrl, filesList, onSynced } from "../lib/core/index.js";
  import { filesNewUrl, filesShowUrl } from "../lib/routes.js";
  import { fileSize } from "./file_size.js";
  import { Badge, Button, ErrorMessage, Heading, Link, Panel, Text } from "../lib/components/index.js";
  import Page from "../lib/Page.svelte";

  let files = $state([]);
  let error = $state("");
  /** @type {{count: number, bytes: number} | null} */
  let cache = $state(null);

  async function load() {
    try {
      files = await filesList();
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

  $effect(() => onSynced(load));
  load();
</script>

<Page>
  <div class="flex items-center justify-between gap-4">
    <Heading level={1}>Files</Heading>
    <Button variant="primary" size="sm" href={filesNewUrl()}>New files</Button>
  </div>

  <ErrorMessage message={error} />

  {#if files.length === 0 && !error}
    <Text tone="muted">No files yet.</Text>
  {:else}
    <AnimationsToggle />
  {/if}

  <ul class="flex flex-col gap-3">
    {#each files as { file, communities, shown_as, state, error: problem } (file.id)}
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
