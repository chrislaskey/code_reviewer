<!-- Writing a post, at /posts/new; done, it shows the post. On a server
     with a single community it goes in the current one. -->
<script>
  import { c, t } from "ttag";
  import { postsCreate } from "../lib/core/index.js";
  import { currentCommunityLoad } from "../lib/current_community.svelte.js";
  import { currentServerHasSingleCommunity, currentServerLoad } from "../lib/current_server.svelte.js";
  import { errorMessageOf } from "../lib/error_message.js";
  import { navigateToUrl } from "../lib/router.js";
  import { postsShowUrl, postsIndexUrl } from "../lib/routes.js";
  import PostsForm from "./components/PostsForm.svelte";
  import { Heading, Link, Page } from "../lib/components/index.js";

  let text = $state("");
  let communityIds = $state([]);
  let error = $state("");
  let busy = $state(false);

  async function load() {
    try {
      await currentServerLoad();
      if (!currentServerHasSingleCommunity()) return;
      const current = await currentCommunityLoad();
      if (current && communityIds.length === 0) communityIds = [current.id];
    } catch {
      // The post is the person's alone
    }
  }

  async function save() {
    busy = true;
    try {
      const post = await postsCreate(text, communityIds);
      await navigateToUrl(postsShowUrl(post.id));
    } catch (problem) {
      error = errorMessageOf(problem);
    } finally {
      busy = false;
    }
  }

  load();
</script>

<Page>
  <Heading level={1}>{t`New post`}</Heading>
  <PostsForm bind:text bind:communityIds submitText={c("verb").t`Post`} {error} {busy} onsubmit={save} />
  <Link href={postsIndexUrl()}>{t`Back to posts`}</Link>
</Page>
