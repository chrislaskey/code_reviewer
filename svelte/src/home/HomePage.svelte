<!-- The app's home page, at /home -->
<script>
  import { jt, t } from "ttag";
  import { postsIndexUrl } from "../lib/routes.js";
  import { Heading, Link, Page, Text, TranslatedParts } from "../lib/components/index.js";
</script>

{#snippet postsLink()}<Link href={postsIndexUrl()}>{t`posts`}</Link>{/snippet}

<Page>
  <!-- untranslated: the app's name -->
  <Heading level={1}>Starter Pack</Heading>
  <Text>
    {t`The same pages on the web, on the desktop and on a phone, over the same Rust application.`}
    <TranslatedParts parts={jt`Log in, then start with the ${postsLink}.`} />
  </Text>
</Page>
