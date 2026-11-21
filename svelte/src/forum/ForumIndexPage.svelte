<!-- The forum app's landing page, at /forum: a placeholder until the
     forum is built, see docs/design/multi-app.md. Online only, so a local user
     or a trial user is told it needs an account. -->
<script>
  import AccountsNeeded from "../lib/AccountsNeeded.svelte";
  import { currentUser } from "../lib/current_user.svelte.js";
  import { Heading, Page, Text } from "../lib/components/index.js";

</script>

<Page>
  <Heading level={1}>Forum</Heading>
  {#if currentUser.value && currentUser.value.kind !== "full"}
    <AccountsNeeded kind={currentUser.value.kind} />
  {:else}
    <Text tone="muted">Nothing here yet.</Text>
  {/if}
</Page>
