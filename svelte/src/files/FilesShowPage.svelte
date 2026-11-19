<!-- One file, at /files/:id: who created it and its communities among
     the person's; its bytes shown as the server says its type is shown,
     an image in place, a text or PDF file in a frame, anything else by
     its name and size; with Download, Edit and Delete, which takes it
     out of the person's communities. A click on an image copies it to
     the clipboard, where the device can. -->
<script>
  import AnimationsToggle from "../lib/AnimationsToggle.svelte";
  import FileImage from "../lib/FileImage.svelte";
  import {
    filesContentUrl,
    filesCopy,
    filesCopySupported,
    filesDelete,
    filesDownload,
    filesDownloadSupported,
    filesGet,
    onSynced,
  } from "../lib/core/index.js";
  import { navigateToUrl, pathParams } from "../lib/router.js";
  import { conflictsIndexUrl, filesEditUrl, filesIndexUrl } from "../lib/routes.js";
  import { fileSize } from "./file_size.js";
  import { Button, ErrorMessage, Heading, Link, Text } from "../lib/components/index.js";
  import Page from "../lib/Page.svelte";

  const id = pathParams().id;
  let shown = $state();
  let canDownload = $state(false);
  let canCopy = $state(false);
  let copied = $state(false);
  let saved = $state("");
  let error = $state("");

  async function load() {
    try {
      [shown, canDownload, canCopy] = await Promise.all([filesGet(id), filesDownloadSupported(), filesCopySupported()]);
    } catch (problem) {
      error = String(problem);
    }
  }

  async function copy() {
    error = "";
    try {
      await filesCopy(id);
      copied = true;
      setTimeout(() => (copied = false), 2000);
    } catch (problem) {
      error = String(problem);
    }
  }

  async function download() {
    error = "";
    try {
      const path = await filesDownload(id);
      saved = path ? `Saved to ${path}` : "";
    } catch (problem) {
      error = String(problem);
    }
  }

  async function remove() {
    if (!confirm("Delete this file from your communities? A community you are not in keeps it.")) return;
    try {
      await filesDelete(id);
      await navigateToUrl(filesIndexUrl());
    } catch (problem) {
      error = String(problem);
    }
  }

  $effect(() => onSynced(load));
  load();
</script>

<Page>
  <Heading level={1}>{shown ? shown.file.name : "File"}</Heading>

  <ErrorMessage message={error} />
  {#if shown}
    <Text tone="muted">
      {shown.creator ? shown.creator.name : "A former member"} · in {shown.communities
        .map((community) => community.name)
        .join(", ")}
    </Text>
    <Text tone="muted">
      {shown.file.content_type} · {fileSize(shown.file.size)} · {shown.file.inserted_at}
      {#if shown.state === "waiting"}
        · waiting to upload
      {:else if shown.state === "refused"}
        · not saved on the server
      {/if}
    </Text>
    {#if shown.error}
      <Text tone="error" size="sm">{shown.error}</Text>
    {/if}
    {#if shown.state === "refused"}
      <Text><Link href={conflictsIndexUrl()}>Try again or let it go</Link></Text>
    {/if}

    {#if shown.shown_as === "image"}
      <AnimationsToggle />
      {#if canCopy}
        <button type="button" class="cursor-copy self-start text-left" title="Copy to the clipboard" onclick={copy}>
          <FileImage src={filesContentUrl(id)} alt={shown.file.name} class="file-shown" />
        </button>
        <Text tone="muted">{copied ? "Copied" : "Click the image to copy it"}</Text>
      {:else}
        <div>
          <FileImage src={filesContentUrl(id)} alt={shown.file.name} class="file-shown" />
        </div>
      {/if}
    {:else if shown.shown_as === "text"}
      <iframe src={filesContentUrl(id)} title={shown.file.name} class="file-shown" sandbox=""></iframe>
    {:else if shown.shown_as === "pdf"}
      <iframe src={filesContentUrl(id)} title={shown.file.name} class="file-shown"></iframe>
    {:else}
      <Text tone="muted">This kind of file is not shown here.</Text>
    {/if}

    <div class="flex flex-wrap items-center gap-2">
      {#if canDownload}
        <Button size="sm" onclick={download}>Download</Button>
      {/if}
      <Button size="sm" href={filesEditUrl(id)}>Edit</Button>
      <Button size="sm" variant="ghost" onclick={remove}>Delete</Button>
      {#if saved}
        <Text as="span" tone="muted">{saved}</Text>
      {/if}
    </div>
  {/if}

  <Link href={filesIndexUrl()}>Back to files</Link>
</Page>
