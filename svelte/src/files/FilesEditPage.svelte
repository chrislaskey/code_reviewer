<!-- Editing a file, at /files/:id/edit: its name, and its communities
     among the person's; its bytes never change. Done, it shows the file -->
<script>
  import { t } from "ttag";
  import { FormError, filesGet, filesUpdate } from "../lib/core/index.js";
  import { errorMessageOf } from "../lib/error_message.js";
  import { navigateToUrl, pathParams } from "../lib/router.js";
  import { filesShowUrl } from "../lib/routes.js";
  import { Button, ErrorMessage, FieldErrors, Heading, Input, Label, Link, Page } from "../lib/components/index.js";
  import FilesCommunitiesField from "./components/FilesCommunitiesField.svelte";

  const id = pathParams().id;
  let loaded = $state(false);
  let name = $state("");
  /** @type {string[]} */
  let communityIds = $state([]);
  /** @type {Record<string, string[]>} */
  let errors = $state({});
  let error = $state("");
  let busy = $state(false);

  async function load() {
    try {
      const shown = await filesGet(id);
      name = shown.file.name;
      communityIds = shown.communities.map((community) => community.id);
      loaded = true;
    } catch (problem) {
      error = errorMessageOf(problem);
    }
  }

  /** @param {SubmitEvent} event */
  async function save(event) {
    event.preventDefault();
    busy = true;
    errors = {};
    error = "";
    try {
      await filesUpdate(id, name, communityIds);
      await navigateToUrl(filesShowUrl(id));
    } catch (problem) {
      if (problem instanceof FormError) errors = problem.errors;
      error = errorMessageOf(problem);
    } finally {
      busy = false;
    }
  }

  load();
</script>

<Page>
  <Heading level={1}>{t`Edit file`}</Heading>
  {#if loaded}
    <form onsubmit={save} class="flex flex-col gap-3">
      <Label text={t`Name`}>
        <Input bind:value={name} disabled={busy} />
        <FieldErrors {errors} fields={["name"]} />
      </Label>
      <FilesCommunitiesField bind:communityIds {errors} disabled={busy} />
      {#if error && Object.keys(errors).length === 0}
        <ErrorMessage message={error} />
      {/if}
      <div>
        <Button variant="primary" type="submit" disabled={busy}>{t`Save`}</Button>
      </div>
    </form>
  {:else if error}
    <ErrorMessage message={error} />
  {/if}
  <Link href={filesShowUrl(id)}>{t`Back to the file`}</Link>
</Page>
