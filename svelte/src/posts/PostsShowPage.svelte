<!-- One post, at /posts/:id, with its Edit and Delete for its owner -->
<script>
  import { t } from "ttag";
  import { dateTimeFormat } from "../lib/formats.js";
  import { postsDelete, postsGet } from "../lib/core/index.js";
  import { loaded } from "../lib/loaded.svelte.js";
  import { currentServerCommunitiesSingle } from "../lib/current_server.svelte.js";
  import { errorMessageOf } from "../lib/error_message.js";
  import { currentUser } from "../lib/current_user.svelte.js";
  import { navigateToUrl, pathParams } from "../lib/router.js";
  import { postsEditUrl, postsIndexUrl } from "../lib/routes.js";
  import { Button, ErrorMessage, Heading, Link, Page, Panel, Text } from "../lib/components/index.js";

  const id = pathParams().id;
  const shown = loaded(() => postsGet(id), { reloadOnSync: true });
  let error = $state("");

  async function remove() {
    if (!confirm(t`Delete this post?`)) return;
    try {
      await postsDelete(id);
      await navigateToUrl(postsIndexUrl());
    } catch (problem) {
      error = errorMessageOf(problem);
    }
  }
</script>

<Page>
  <Heading level={1}>{t`Post`}</Heading>

  <ErrorMessage message={error || shown.error} />
  {#if shown.value}
    {@const { post, owner, communities } = shown.value}
    <Panel as="article">
      <Text tone="muted">
        {owner.name} · {dateTimeFormat(post.inserted_at)}
        {#if communities.length > 0 && !currentServerCommunitiesSingle()}
          {@const communityNames = communities.map((community) => community.name).join(", ")}
          · {t`in ${communityNames}`}
        {/if}
      </Text>
      <Text class="whitespace-pre-wrap">{post.body}</Text>
    </Panel>
    {#if post.user_id === currentUser.value?.id}
      <div class="flex gap-2">
        <Button size="sm" href={postsEditUrl(post.id)}>{t`Edit`}</Button>
        <Button size="sm" variant="ghost" onclick={remove}>{t`Delete`}</Button>
      </div>
    {/if}
  {/if}

  <Link href={postsIndexUrl()}>{t`Back to posts`}</Link>
</Page>
