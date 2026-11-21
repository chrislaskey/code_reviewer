<!-- Editing a community, at /communities/:id/edit; done, it shows the
     community -->
<script>
  import { FormError, communitiesGet, communitiesUpdate } from "../lib/core/index.js";
  import { errorMessageOf } from "../lib/error_message.js";
  import { navigateToUrl, pathParams } from "../lib/router.js";
  import { communitiesShowUrl } from "../lib/routes.js";
  import CommunitiesForm from "./components/CommunitiesForm.svelte";
  import { ErrorMessage, Heading, Link, Page } from "../lib/components/index.js";

  const id = pathParams().id;
  let loaded = $state(false);
  let name = $state("");
  /** @type {Record<string, string[]>} */
  let errors = $state({});
  let error = $state("");
  let busy = $state(false);

  async function load() {
    try {
      name = (await communitiesGet(id)).community.name;
      loaded = true;
    } catch (problem) {
      error = errorMessageOf(problem);
    }
  }

  async function save() {
    busy = true;
    errors = {};
    error = "";
    try {
      await communitiesUpdate(id, name);
      await navigateToUrl(communitiesShowUrl(id));
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
  <Heading level={1}>Edit community</Heading>
  {#if loaded}
    <CommunitiesForm bind:name submitText="Save" {errors} {error} {busy} onsubmit={save} />
  {:else if error}
    <ErrorMessage message={error} />
  {/if}
  <Link href={communitiesShowUrl(id)}>Back to the community</Link>
</Page>
