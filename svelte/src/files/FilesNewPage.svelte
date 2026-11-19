<!-- Creating a file, at /files/new: the person picks one, names it, its
     own name to start, and picks its communities, their current one to
     start; done, it shows the file. A file the person already sees with
     the same bytes is shown once picked: already in a community chosen,
     or offered to put in those chosen instead of a copy. On the web the
     bytes go straight to object storage; in the app they are kept on the
     device and sent when it next syncs, see filesCreate in lib/core/. -->
<script>
  import FieldErrors from "../lib/FieldErrors.svelte";
  import { FormError, filesCreate, filesListDuplicates, filesSha256, filesUpdate } from "../lib/core/index.js";
  import { currentCommunityLoad } from "../lib/current_community.js";
  import { navigateToUrl } from "../lib/router.js";
  import { filesIndexUrl, filesShowUrl } from "../lib/routes.js";
  import { Alert, Button, ErrorMessage, FileInput, Heading, Input, Label, Link, Text } from "../lib/components/index.js";
  import Page from "../lib/Page.svelte";
  import FilesCommunitiesField from "./FilesCommunitiesField.svelte";

  /** @type {File | null} */
  let file = $state(null);
  let name = $state("");
  /** @type {string[]} */
  let communityIds = $state([]);
  /** @type {Record<string, string[]>} */
  let errors = $state({});
  let error = $state("");
  let busy = $state(false);
  /** From 0 to 1 while the file uploads, else null. */
  let progress = $state(null);
  /** The newest file the person sees with the same bytes, once picked. */
  let same = $state(null);
  let sameInChosen = $derived(same?.communities.some((community) => communityIds.includes(community.id)) ?? false);

  /** @param {Event & {currentTarget: HTMLInputElement}} event */
  async function pick(event) {
    file = event.currentTarget.files?.[0] ?? null;
    name = file?.name ?? "";
    errors = {};
    error = "";
    same = null;
    const picked = file;
    if (!picked) return;
    try {
      const found = await filesListDuplicates([await filesSha256(picked)]);
      if (file === picked) same = found[0] ?? null;
    } catch {
      // Creating a copy is still fine
    }
  }

  async function putSameInChosen() {
    busy = true;
    error = "";
    try {
      const mine = same.communities.map((community) => community.id);
      const ids = [...mine, ...communityIds.filter((id) => !mine.includes(id))];
      await filesUpdate(same.file.id, same.file.name, ids);
      await navigateToUrl(filesShowUrl(same.file.id));
    } catch (problem) {
      error = problem instanceof Error ? problem.message : String(problem);
    } finally {
      busy = false;
    }
  }

  /** @param {SubmitEvent} event */
  async function save(event) {
    event.preventDefault();
    if (!file) return;
    busy = true;
    errors = {};
    error = "";
    try {
      const created = await filesCreate(file, name, communityIds, (fraction) => (progress = fraction));
      await navigateToUrl(filesShowUrl(created.id));
    } catch (problem) {
      if (problem instanceof FormError) errors = problem.errors;
      error = problem instanceof Error ? problem.message : String(problem);
    } finally {
      busy = false;
      progress = null;
    }
  }

  async function load() {
    try {
      const current = await currentCommunityLoad();
      if (current && communityIds.length === 0) communityIds = [current.id];
    } catch {
      // The person picks one
    }
  }

  load();
</script>

<Page>
  <Heading level={1}>New file</Heading>
  <form onsubmit={save} class="flex flex-col gap-3">
    <Label text="File">
      <FileInput onchange={pick} disabled={busy} />
      <FieldErrors {errors} fields={["content_type", "size"]} />
    </Label>
    <Label text="Name">
      <Input bind:value={name} disabled={busy} />
      <FieldErrors {errors} fields={["name"]} />
    </Label>
    <FilesCommunitiesField bind:communityIds {errors} disabled={busy} />
    {#if same}
      {@const communityNames = same.communities.map((community) => community.name).join(", ")}
      <Alert class="flex flex-col items-start gap-2">
        <Text>
          This file is already here, as <Link href={filesShowUrl(same.file.id)}>{same.file.name}</Link> in {communityNames}.
        </Text>
        {#if !sameInChosen && communityIds.length > 0}
          <Button size="sm" disabled={busy} onclick={putSameInChosen}>Put it in the communities chosen instead</Button>
        {/if}
      </Alert>
    {/if}
    {#if progress !== null}
      <progress class="progress w-full" value={progress} max="1"></progress>
    {/if}
    {#if error && Object.keys(errors).length === 0}
      <ErrorMessage message={error} />
    {/if}
    <div>
      <Button variant="primary" type="submit" disabled={busy || !file}>{same ? "Create a copy" : "Create file"}</Button>
    </div>
  </form>
  <Link href={filesIndexUrl()}>Back to files</Link>
</Page>
