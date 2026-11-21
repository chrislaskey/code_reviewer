<!-- Writing a post, at /posts/new; done, it shows the post -->
<script>
  import { c, t } from "ttag";
  import { postsCreate } from "../lib/core/index.js";
  import { navigateToUrl } from "../lib/router.js";
  import { postsShowUrl, postsIndexUrl } from "../lib/routes.js";
  import PostsForm from "./components/PostsForm.svelte";
  import { Heading, Link, Page } from "../lib/components/index.js";

  let text = $state("");
  let communityIds = $state([]);
  let error = $state("");
  let busy = $state(false);

  async function save() {
    busy = true;
    try {
      const post = await postsCreate(text, communityIds);
      await navigateToUrl(postsShowUrl(post.id));
    } catch (problem) {
      error = String(problem);
    } finally {
      busy = false;
    }
  }
</script>

<Page>
  <Heading level={1}>{t`New post`}</Heading>
  <PostsForm bind:text bind:communityIds submitText={c("verb").t`Post`} {error} {busy} onsubmit={save} />
  <Link href={postsIndexUrl()}>{t`Back to posts`}</Link>
</Page>
