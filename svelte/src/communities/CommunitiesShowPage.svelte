<!-- One community, at /communities/:id, to join or leave, and for a
     member to edit or delete -->
<script>
  import { t } from "ttag";
  import { dateFormat } from "../lib/formats.js";
  import { communitiesDelete, communitiesGet } from "../lib/core/index.js";
  import { loaded } from "../lib/loaded.svelte.js";
  import { errorMessageOf } from "../lib/error_message.js";
  import { navigateToUrl, pathParams } from "../lib/router.js";
  import { communitiesEditUrl, communitiesIndexUrl } from "../lib/routes.js";
  import CommunitiesDeleteDialog from "./components/CommunitiesDeleteDialog.svelte";
  import CommunitiesMembershipButton from "./components/CommunitiesMembershipButton.svelte";
  import { Button, ErrorMessage, Heading, Link, Page, Text } from "../lib/components/index.js";

  const id = pathParams().id;
  const shown = loaded(() => communitiesGet(id), { reloadOnSync: true });
  let error = $state("");
  let deleting = $state(false);
  let busy = $state(false);

  async function remove() {
    busy = true;
    try {
      await communitiesDelete(id);
      deleting = false;
      await navigateToUrl(communitiesIndexUrl());
    } catch (problem) {
      deleting = false;
      error = errorMessageOf(problem);
    } finally {
      busy = false;
    }
  }
</script>

<Page>
  <Heading level={1}>{shown.value?.community.name ?? t`Community`}</Heading>

  <ErrorMessage message={error || shown.error} />
  {#if shown.value}
    {@const { community, membership } = shown.value}
    {@const createdOn = dateFormat(community.inserted_at)}
    <Text tone="muted">
      {t`Created ${createdOn}`}
      {#if membership}
        · {t`you are a member`}
      {/if}
    </Text>
    <div class="flex flex-wrap gap-2">
      <CommunitiesMembershipButton
        communityId={id}
        {membership}
        onchange={shown.reload}
        onerror={(problem) => (error = problem)}
      />
      {#if membership}
        <Button size="sm" href={communitiesEditUrl(id)}>{t`Edit`}</Button>
        <Button size="sm" variant="ghost" onclick={() => (deleting = true)}>{t`Delete`}</Button>
      {/if}
    </div>
    <CommunitiesDeleteDialog
      community={deleting ? community : null}
      {busy}
      ondelete={remove}
      oncancel={() => (deleting = false)}
    />
  {/if}

  <Link href={communitiesIndexUrl()}>{t`Back to communities`}</Link>
</Page>
