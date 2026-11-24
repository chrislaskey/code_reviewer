<!-- Every post the person sees, newest first, at /posts: their own, and
     those in their communities -->
<script>
  import { t } from "ttag";
  import { dateTimeFormat } from "../lib/formats.js";
  import { postsList } from "../lib/core/index.js";
  import { loaded } from "../lib/loaded.svelte.js";
  import { currentServerHasSingleCommunity } from "../lib/current_server.svelte.js";
  import { postsNewUrl, postsShowUrl } from "../lib/routes.js";
  import { Button, ErrorMessage, Heading, Link, Page, Panel, Text } from "../lib/components/index.js";

  const posts = loaded(postsList, { reloadOnSync: true });
</script>

<Page>
  <div class="flex items-center justify-between gap-4">
    <Heading level={1}>{t`Posts`}</Heading>
    <Button variant="primary" size="sm" href={postsNewUrl()}>{t`New post`}</Button>
  </div>

  <ErrorMessage message={posts.error} />

  {#if posts.value?.length === 0}
    <Text tone="muted">{t`No posts yet.`}</Text>
  {/if}

  <ul class="flex flex-col gap-3">
    {#each posts.value ?? [] as { post, owner, communities } (post.id)}
      <Panel as="li">
        <Text tone="muted">
          {owner.name} · {dateTimeFormat(post.inserted_at)}
          {#if communities.length > 0 && !currentServerHasSingleCommunity()}
            {@const communityNames = communities.map((community) => community.name).join(", ")}
            · {t`in ${communityNames}`}
          {/if}
        </Text>
        <Text class="whitespace-pre-wrap">{post.body}</Text>
        <Link href={postsShowUrl(post.id)}>{t`Show`}</Link>
      </Panel>
    {/each}
  </ul>
</Page>
