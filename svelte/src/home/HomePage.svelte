<!-- The app's home page, at /home -->
<script>
  import { postsIndexUrl } from "../lib/routes.js";
  import { Heading, Link, Page, Text } from "../lib/components/index.js";
</script>

<Page>
  <Heading level={1}>Starter Pack</Heading>
  <Text>
    The same pages on the web, on the desktop and on a phone, over the same Rust application.
    Log in, then start with the <Link href={postsIndexUrl()}>posts</Link>.
  </Text>
</Page>
