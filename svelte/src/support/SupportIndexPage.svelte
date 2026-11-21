<!-- The support app's landing page, at /support: a placeholder until
     support is built, see docs/design/multi-app.md. Online only, so a local user
     or a trial user is told it needs an account. -->
<script>
  import AccountsNeeded from "../lib/AccountsNeeded.svelte";
  import { currentUserLoad } from "../lib/current_user.js";
  import { Heading, Page, Text } from "../lib/components/index.js";

  /** @type {import("../lib/current_user.js").User | null} */
  let currentUser = $state(null);

  currentUserLoad().then(
    (user) => (currentUser = user),
    () => (currentUser = null),
  );
</script>

<Page>
  <Heading level={1}>Support</Heading>
  {#if currentUser && currentUser.kind !== "full"}
    <AccountsNeeded kind={currentUser.kind} />
  {:else}
    <Text tone="muted">Nothing here yet.</Text>
  {/if}
</Page>
