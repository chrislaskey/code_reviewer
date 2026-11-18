<!-- One community, at /communities/:id, to join or leave, and for a
     member to edit or delete -->
<script>
  import { communitiesDelete, communitiesGet, onSynced } from "../lib/core/index.js";
  import { navigateToUrl, pathParams } from "../lib/router.js";
  import { communitiesEditUrl, communitiesIndexUrl } from "../lib/routes.js";
  import CommunitiesDeleteDialog from "./CommunitiesDeleteDialog.svelte";
  import CommunitiesMembershipButton from "./CommunitiesMembershipButton.svelte";
  import { Button, ErrorMessage, Heading, Link, Text } from "../lib/components/index.js";
  import Page from "../lib/Page.svelte";

  const id = pathParams().id;
  let shown = $state();
  let error = $state("");
  let deleting = $state(false);
  let busy = $state(false);

  async function load() {
    try {
      shown = await communitiesGet(id);
      error = "";
    } catch (problem) {
      error = String(problem);
    }
  }

  async function remove() {
    busy = true;
    try {
      await communitiesDelete(id);
      deleting = false;
      await navigateToUrl(communitiesIndexUrl());
    } catch (problem) {
      deleting = false;
      error = String(problem);
    } finally {
      busy = false;
    }
  }

  $effect(() => onSynced(load));
  load();
</script>

<Page>
  <Heading level={1}>{shown ? shown.community.name : "Community"}</Heading>

  <ErrorMessage message={error} />
  {#if shown}
    <Text tone="muted">
      Created {shown.community.inserted_at}
      {#if shown.membership}
        · you are a member
      {/if}
    </Text>
    <div class="flex flex-wrap gap-2">
      <CommunitiesMembershipButton
        communityId={id}
        membership={shown.membership}
        onchange={load}
        onerror={(problem) => (error = problem)}
      />
      {#if shown.membership}
        <Button size="sm" href={communitiesEditUrl(id)}>Edit</Button>
        <Button size="sm" variant="ghost" onclick={() => (deleting = true)}>Delete</Button>
      {/if}
    </div>
    <CommunitiesDeleteDialog
      community={deleting ? shown.community : null}
      {busy}
      ondelete={remove}
      oncancel={() => (deleting = false)}
    />
  {/if}

  <Link href={communitiesIndexUrl()}>Back to communities</Link>
</Page>
