<!-- Creating a file, at /files/new: the person picks one, names it, its
     own name to start, and picks its communities, their current one to
     start; done, it shows the file. On the web the bytes go straight to
     object storage; in the app they are kept on the device and sent when
     it next syncs, see filesCreate in lib/core/. -->
<script>
  import FieldErrors from "../lib/FieldErrors.svelte";
  import { FormError, filesCreate } from "../lib/core/index.js";
  import { currentCommunityLoad } from "../lib/current_community.js";
  import { navigateToUrl } from "../lib/router.js";
  import { filesIndexUrl, filesShowUrl } from "../lib/routes.js";
  import { Button, ErrorMessage, FileInput, Heading, Input, Label, Link } from "../lib/components/index.js";
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

  /** @param {Event & {currentTarget: HTMLInputElement}} event */
  function pick(event) {
    file = event.currentTarget.files?.[0] ?? null;
    name = file?.name ?? "";
    errors = {};
    error = "";
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
    {#if progress !== null}
      <progress class="progress w-full" value={progress} max="1"></progress>
    {/if}
    {#if error && Object.keys(errors).length === 0}
      <ErrorMessage message={error} />
    {/if}
    <div>
      <Button variant="primary" type="submit" disabled={busy || !file}>Create file</Button>
    </div>
  </form>
  <Link href={filesIndexUrl()}>Back to files</Link>
</Page>
