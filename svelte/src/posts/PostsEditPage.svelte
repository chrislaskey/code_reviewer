<!-- Editing a post, at /posts/:id/edit; done, it shows the post -->
<script>
  import { t } from "ttag";
  import AppNav from "../lib/AppNav.svelte";
  import { postsGet, postsUpdate } from "../lib/core/index.js";
  import { navigateToUrl, pathParams } from "../lib/router.js";
  import { postsShowUrl } from "../lib/routes.js";
  import PostsForm from "./PostsForm.svelte";

  const id = pathParams().id;
  let loaded = $state(false);
  let text = $state("");
  let communityIds = $state([]);
  let error = $state("");
  let busy = $state(false);

  async function load() {
    try {
      const { post, communities } = await postsGet(id);
      text = post.body;
      communityIds = communities.map((community) => community.id);
      loaded = true;
    } catch (problem) {
      error = String(problem);
    }
  }

  async function save() {
    busy = true;
    try {
      await postsUpdate(id, text, communityIds);
      await navigateToUrl(postsShowUrl(id));
    } catch (problem) {
      error = String(problem);
    } finally {
      busy = false;
    }
  }

  load();
</script>

<main class="page">
  <AppNav />
  <h1>{t`Edit post`}</h1>
  {#if loaded}
    <PostsForm bind:text bind:communityIds submitText={t`Save`} {error} {busy} onsubmit={save} />
  {:else if error}
    <p class="error">{error}</p>
  {/if}
  <a href={postsShowUrl(id)}>{t`Back to the post`}</a>
</main>
