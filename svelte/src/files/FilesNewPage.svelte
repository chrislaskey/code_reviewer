<!-- Adding a file, at /files/new: the person picks one and names it, its
     own name to start; done, it shows the file. On the web the bytes go
     straight to object storage; in the app they are kept on the device
     and sent when it next syncs, see filesCreate in lib/core/. -->
<script>
  import FieldErrors from "../lib/FieldErrors.svelte";
  import { FormError, filesCreate } from "../lib/core/index.js";
  import { navigateToUrl } from "../lib/router.js";
  import { filesIndexUrl, filesShowUrl } from "../lib/routes.js";
  import { Button, ErrorMessage, FileInput, Heading, Input, Label, Link } from "../lib/components/index.js";
  import Page from "../lib/Page.svelte";

  /** @type {File | null} */
  let file = $state(null);
  let name = $state("");
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
      const added = await filesCreate(file, name, (fraction) => (progress = fraction));
      await navigateToUrl(filesShowUrl(added.id));
    } catch (problem) {
      if (problem instanceof FormError) errors = problem.errors;
      error = problem instanceof Error ? problem.message : String(problem);
    } finally {
      busy = false;
      progress = null;
    }
  }
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
    {#if progress !== null}
      <progress class="progress w-full" value={progress} max="1"></progress>
    {/if}
    {#if error && Object.keys(errors).length === 0}
      <ErrorMessage message={error} />
    {/if}
    <div>
      <Button variant="primary" type="submit" disabled={busy || !file}>Add file</Button>
    </div>
  </form>
  <Link href={filesIndexUrl()}>Back to files</Link>
</Page>
