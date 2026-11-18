<!-- Where Stripe's pages come back to when the desktop app opened them,
     at /accounts/back-to-app: open to anyone, as this browser may not be
     signed in. The app asks the server again when it has the focus. See
     docs/design/purchasing.md, step 7. -->
<script>
  import { Heading, Text } from "../lib/components/index.js";
  import Page from "../lib/Page.svelte";
</script>

<Page>
  <Heading level={1}>All done here</Heading>
  <Text>Go back to the app. It shows what changed when you do.</Text>
  <Text tone="muted">You can close this tab.</Text>
</Page>
