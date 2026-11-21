<!-- Every post the person sees, newest first, at /posts: their own, and
     those in their communities -->
<script>
  import { t } from "ttag";
  import { onSynced, postsList } from "../lib/core/index.js";
  import { postsNewUrl, postsShowUrl } from "../lib/routes.js";
  import { Button, ErrorMessage, Heading, Link, Page, Panel, Text } from "../lib/components/index.js";

  let posts = $state([]);
  let error = $state("");

  async function load() {
    try {
      posts = await postsList();
    } catch (problem) {
      error = String(problem);
    }
  }

  $effect(() => onSynced(load));
  load();
</script>

<Page>
  <div class="flex items-center justify-between gap-4">
    <Heading level={1}>{t`Posts`}</Heading>
    <Button variant="primary" size="sm" href={postsNewUrl()}>{t`New post`}</Button>
  </div>

  <ErrorMessage message={error} />

  {#if posts.length === 0 && !error}
    <Text tone="muted">{t`No posts yet.`}</Text>
  {/if}

  <ul class="flex flex-col gap-3">
    {#each posts as { post, owner, communities } (post.id)}
      <Panel as="li">
        <Text tone="muted">
          {owner.name} · {post.inserted_at}
          {#if communities.length > 0}
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
