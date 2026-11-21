<!-- One post, at /posts/:id, with its Edit and Delete for its owner -->
<script>
  import { t } from "ttag";
  import { onSynced, postsDelete, postsGet } from "../lib/core/index.js";
  import { currentUserLoad } from "../lib/current_user.js";
  import { navigateToUrl, pathParams } from "../lib/router.js";
  import { postsEditUrl, postsIndexUrl } from "../lib/routes.js";
  import { Button, ErrorMessage, Heading, Link, Page, Panel, Text } from "../lib/components/index.js";

  const id = pathParams().id;
  let shown = $state();
  let currentUser = $state();
  let error = $state("");

  async function load() {
    try {
      [shown, currentUser] = await Promise.all([postsGet(id), currentUserLoad()]);
    } catch (problem) {
      error = String(problem);
    }
  }

  async function remove() {
    if (!confirm(t`Delete this post?`)) return;
    try {
      await postsDelete(id);
      await navigateToUrl(postsIndexUrl());
    } catch (problem) {
      error = String(problem);
    }
  }

  $effect(() => onSynced(load));
  load();
</script>

<Page>
  <Heading level={1}>{t`Post`}</Heading>

  <ErrorMessage message={error} />
  {#if shown}
    <Panel as="article">
      <Text tone="muted">
        {shown.owner.name} · {shown.post.inserted_at}
        {#if shown.communities.length > 0}
          {@const communityNames = shown.communities.map((community) => community.name).join(", ")}
          · {t`in ${communityNames}`}
        {/if}
      </Text>
      <Text class="whitespace-pre-wrap">{shown.post.body}</Text>
    </Panel>
    {#if currentUser && shown.post.user_id === currentUser.id}
      <div class="flex gap-2">
        <Button size="sm" href={postsEditUrl(shown.post.id)}>{t`Edit`}</Button>
        <Button size="sm" variant="ghost" onclick={remove}>{t`Delete`}</Button>
      </div>
    {/if}
  {/if}

  <Link href={postsIndexUrl()}>{t`Back to posts`}</Link>
</Page>
