<!-- Every community, at /communities, each to show, join or leave -->
<script>
  import { communitiesList, onSynced } from "../lib/core/index.js";
  import { communitiesNewUrl, communitiesShowUrl } from "../lib/routes.js";
  import CommunitiesMembershipButton from "./components/CommunitiesMembershipButton.svelte";
  import { Button, ErrorMessage, Heading, Link, Page, Panel, Text } from "../lib/components/index.js";

  let communities = $state([]);
  let error = $state("");

  async function load() {
    try {
      communities = await communitiesList();
      error = "";
    } catch (problem) {
      error = String(problem);
    }
  }

  $effect(() => onSynced(load));
  load();
</script>

<Page>
  <div class="flex items-center justify-between gap-4">
    <Heading level={1}>Communities</Heading>
    <Button variant="primary" size="sm" href={communitiesNewUrl()}>New community</Button>
  </div>

  <ErrorMessage message={error} />

  {#if communities.length === 0 && !error}
    <Text tone="muted">No communities yet.</Text>
  {/if}

  <ul class="flex flex-col gap-3">
    {#each communities as { community, membership } (community.id)}
      <Panel as="li" layout="row" class="justify-between gap-4">
        <span>
          <Link href={communitiesShowUrl(community.id)}>{community.name}</Link>
          {#if membership}
            <Text as="span" tone="muted">· member</Text>
          {/if}
        </span>
        <CommunitiesMembershipButton
          communityId={community.id}
          {membership}
          onchange={load}
          onerror={(problem) => (error = problem)}
        />
      </Panel>
    {/each}
  </ul>
</Page>
