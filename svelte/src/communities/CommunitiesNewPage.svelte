<!-- Making a community, at /communities/new, which the person joins;
     done, it shows the community -->
<script>
  import { FormError, communitiesCreate } from "../lib/core/index.js";
  import { navigateToUrl } from "../lib/router.js";
  import { communitiesIndexUrl, communitiesShowUrl } from "../lib/routes.js";
  import CommunitiesForm from "./CommunitiesForm.svelte";
  import { Heading, Link } from "../lib/components/index.js";
  import Page from "../lib/Page.svelte";

  let name = $state("");
  /** @type {Record<string, string[]>} */
  let errors = $state({});
  let error = $state("");
  let busy = $state(false);

  async function save() {
    busy = true;
    errors = {};
    error = "";
    try {
      const community = await communitiesCreate(name);
      await navigateToUrl(communitiesShowUrl(community.id));
    } catch (problem) {
      if (problem instanceof FormError) errors = problem.errors;
      error = problem instanceof Error ? problem.message : String(problem);
    } finally {
      busy = false;
    }
  }
</script>

<Page>
  <Heading level={1}>New community</Heading>
  <CommunitiesForm bind:name submitText="Create" {errors} {error} {busy} onsubmit={save} />
  <Link href={communitiesIndexUrl()}>Back to communities</Link>
</Page>
