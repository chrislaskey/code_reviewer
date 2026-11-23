<!-- Making a community, at /communities/new, which the person joins;
     done, it shows the community -->
<script>
  import { t } from "ttag";
  import { FormError, communitiesCreate } from "../lib/core/index.js";
  import { errorMessageOf } from "../lib/error_message.js";
  import { navigateToUrl } from "../lib/router.js";
  import { communitiesIndexUrl, communitiesShowUrl } from "../lib/routes.js";
  import CommunitiesForm from "./components/CommunitiesForm.svelte";
  import { Heading, Link, Page } from "../lib/components/index.js";

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
      error = errorMessageOf(problem);
    } finally {
      busy = false;
    }
  }
</script>

<Page>
  <Heading level={1}>{t`New community`}</Heading>
  <CommunitiesForm bind:name submitText={t`Create`} {errors} {error} {busy} onsubmit={save} />
  <Link href={communitiesIndexUrl()}>{t`Back to communities`}</Link>
</Page>
