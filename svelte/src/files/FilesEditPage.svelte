<!-- Renaming a file, at /files/:id/edit, the one thing about a file
     that changes; done, it shows the file -->
<script>
  import FieldErrors from "../lib/FieldErrors.svelte";
  import { FormError, filesGet, filesUpdate } from "../lib/core/index.js";
  import { navigateToUrl, pathParams } from "../lib/router.js";
  import { filesShowUrl } from "../lib/routes.js";
  import { Button, ErrorMessage, Heading, Input, Label, Link } from "../lib/components/index.js";
  import Page from "../lib/Page.svelte";

  const id = pathParams().id;
  let loaded = $state(false);
  let name = $state("");
  /** @type {Record<string, string[]>} */
  let errors = $state({});
  let error = $state("");
  let busy = $state(false);

  async function load() {
    try {
      name = (await filesGet(id)).file.name;
      loaded = true;
    } catch (problem) {
      error = String(problem);
    }
  }

  /** @param {SubmitEvent} event */
  async function save(event) {
    event.preventDefault();
    busy = true;
    errors = {};
    error = "";
    try {
      await filesUpdate(id, name);
      await navigateToUrl(filesShowUrl(id));
    } catch (problem) {
      if (problem instanceof FormError) errors = problem.errors;
      error = problem instanceof Error ? problem.message : String(problem);
    } finally {
      busy = false;
    }
  }

  load();
</script>

<Page>
  <Heading level={1}>Rename file</Heading>
  {#if loaded}
    <form onsubmit={save} class="flex flex-col gap-3">
      <Label text="Name">
        <Input bind:value={name} disabled={busy} />
        <FieldErrors {errors} fields={["name"]} />
      </Label>
      {#if error && Object.keys(errors).length === 0}
        <ErrorMessage message={error} />
      {/if}
      <div>
        <Button variant="primary" type="submit" disabled={busy}>Save</Button>
      </div>
    </form>
  {:else if error}
    <ErrorMessage message={error} />
  {/if}
  <Link href={filesShowUrl(id)}>Back to the file</Link>
</Page>
