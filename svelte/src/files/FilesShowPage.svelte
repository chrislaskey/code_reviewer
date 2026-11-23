<!-- One file, at /files/:id: who created it and its communities among
     the person's; its bytes shown as the server says its type is shown,
     an image in place, a text or PDF file in a frame, anything else by
     its name and size; with Download, Edit and Delete, which takes it
     out of the person's communities. A click on an image copies it to
     the clipboard, where the device can. -->
<script>
  import { t } from "ttag";
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
  } from "../lib/core/index.js";
  import { loaded } from "../lib/loaded.svelte.js";
  import { errorMessageOf } from "../lib/error_message.js";
  import { navigateToUrl, pathParams } from "../lib/router.js";
  import { conflictsIndexUrl, filesEditUrl, filesIndexUrl } from "../lib/routes.js";
  import { fileSize } from "../lib/file_size.js";
  import { Button, ErrorMessage, Heading, Link, Page, Text } from "../lib/components/index.js";

  const id = pathParams().id;
  const file = loaded(() => filesGet(id), { reloadOnSync: true });
  const can = loaded(async () => ({ download: await filesDownloadSupported(), copy: await filesCopySupported() }));
  let shown = $derived(file.value);
  let copied = $state(false);
  let saved = $state("");
  let error = $state("");

  async function copy() {
    error = "";
    try {
      await filesCopy(id);
      copied = true;
      setTimeout(() => (copied = false), 2000);
    } catch (problem) {
      error = errorMessageOf(problem);
    }
  }

  async function download() {
    error = "";
    try {
      const path = await filesDownload(id);
      saved = path ? t`Saved to ${path}` : "";
    } catch (problem) {
      error = errorMessageOf(problem);
    }
  }

  async function remove() {
    if (!confirm(t`Delete this file from your communities? A community you are not in keeps it.`)) return;
    try {
      await filesDelete(id);
      await navigateToUrl(filesIndexUrl());
    } catch (problem) {
      error = errorMessageOf(problem);
    }
  }
</script>

<Page>
  <Heading level={1}>{shown ? shown.file.name : t`File`}</Heading>

  <ErrorMessage message={error || file.error} />
  {#if shown}
    {@const communityNames = shown.communities.map((community) => community.name).join(", ")}
    <Text tone="muted">
      {shown.creator ? shown.creator.name : t`A former member`} · {t`in ${communityNames}`}
    </Text>
    <Text tone="muted">
      {shown.file.content_type} · {fileSize(shown.file.size)} · {shown.file.inserted_at}
      {#if shown.state === "waiting"}
        · {t`waiting to upload`}
      {:else if shown.state === "refused"}
        · {t`not saved on the server`}
      {/if}
    </Text>
    {#if shown.error}
      <Text tone="error" size="sm">{shown.error}</Text>
    {/if}
    {#if shown.state === "refused"}
      <Text><Link href={conflictsIndexUrl()}>{t`Try again or let it go`}</Link></Text>
    {/if}

    {#if shown.shown_as === "image"}
      <AnimationsToggle />
      {#if can.value?.copy}
        <button type="button" class="cursor-copy self-start text-left" title={t`Copy to the clipboard`} onclick={copy}>
          <FileImage src={filesContentUrl(id)} alt={shown.file.name} class="file-shown" />
        </button>
        <Text tone="muted">{copied ? t`Copied` : t`Click the image to copy it`}</Text>
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
      <Text tone="muted">{t`This kind of file is not shown here.`}</Text>
    {/if}

    <div class="flex flex-wrap items-center gap-2">
      {#if can.value?.download}
        <Button size="sm" onclick={download}>{t`Download`}</Button>
      {/if}
      <Button size="sm" href={filesEditUrl(id)}>{t`Edit`}</Button>
      <Button size="sm" variant="ghost" onclick={remove}>{t`Delete`}</Button>
      {#if saved}
        <Text as="span" tone="muted">{saved}</Text>
      {/if}
    </div>
  {/if}

  <Link href={filesIndexUrl()}>{t`Back to files`}</Link>
</Page>
