<!-- Every community, at /communities, each to show, join or leave -->
<script>
  import { t } from "ttag";
  import { communitiesList } from "../lib/core/index.js";
  import { loaded } from "../lib/loaded.svelte.js";
  import { communitiesNewUrl, communitiesShowUrl } from "../lib/routes.js";
  import CommunitiesMembershipButton from "./components/CommunitiesMembershipButton.svelte";
  import { Button, ErrorMessage, Heading, Link, Page, Panel, Text } from "../lib/components/index.js";

  const communities = loaded(communitiesList, { reloadOnSync: true });
  let error = $state("");
</script>

<Page>
  <div class="flex items-center justify-between gap-4">
    <Heading level={1}>{t`Communities`}</Heading>
    <Button variant="primary" size="sm" href={communitiesNewUrl()}>{t`New community`}</Button>
  </div>

  <ErrorMessage message={error || communities.error} />

  {#if communities.value?.length === 0}
    <Text tone="muted">{t`No communities yet.`}</Text>
  {/if}

  <ul class="flex flex-col gap-3">
    {#each communities.value ?? [] as { community, membership } (community.id)}
      <Panel as="li" layout="row" class="justify-between gap-4">
        <span>
          <Link href={communitiesShowUrl(community.id)}>{community.name}</Link>
          {#if membership}
            <Text as="span" tone="muted">· {t`member`}</Text>
          {/if}
        </span>
        <CommunitiesMembershipButton
          communityId={community.id}
          {membership}
          onchange={communities.reload}
          onerror={(problem) => (error = problem)}
        />
      </Panel>
    {/each}
  </ul>
</Page>
