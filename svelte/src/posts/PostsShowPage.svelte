<!-- One post, at /posts/:id, with its Edit and Delete for its owner -->
<script>
  import { t } from "ttag";
  import AppNav from "../lib/AppNav.svelte";
  import { onSynced, postsDelete, postsGet } from "../lib/core/index.js";
  import { currentUserLoad } from "../lib/current_user.js";
  import { navigateToUrl, pathParams } from "../lib/router.js";
  import { postsEditUrl, postsIndexUrl } from "../lib/routes.js";

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

<main class="page">
  <AppNav />
  <h1>{t`Post`}</h1>

  {#if error}
    <p class="error">{error}</p>
  {/if}
  {#if shown}
    <article class="post">
      <p class="muted">
        {shown.owner.name} · {shown.post.inserted_at}
        {#if shown.communities.length > 0}
          {@const communityNames = shown.communities.map((community) => community.name).join(", ")}
          · {t`in ${communityNames}`}
        {/if}
      </p>
      <p class="whitespace-pre-wrap">{shown.post.body}</p>
    </article>
    {#if currentUser && shown.post.user_id === currentUser.id}
      <div class="flex gap-2">
        <a href={postsEditUrl(shown.post.id)} class="btn btn-sm">{t`Edit`}</a>
        <button type="button" class="btn btn-sm btn-ghost" onclick={remove}>{t`Delete`}</button>
      </div>
    {/if}
  {/if}

  <a href={postsIndexUrl()}>{t`Back to posts`}</a>
</main>
