<!-- Shown in place of what needs an account on the server, to someone
     who has none yet: a local user, in the app, or a trial user, on the
     web. See docs/design/accounts-local-and-trial-users.md. -->
<script>
  import { accountsIndexUrl, accountsSignUpUrl } from "../lib/routes.js";
  import { Button, Text } from "../lib/components/index.js";

  /**
   * @typedef {Object} Props
   * @property {"local" | "trial"} kind
   */

  /** @type {Props} */
  let { kind } = $props();
</script>

{#if kind === "local"}
  <Text>This is online only, and your account is only on this device.</Text>
  <Text><Button size="sm" variant="primary" href={accountsIndexUrl()}>Sign up or sign in</Button></Text>
{:else}
  <Text>This needs an account, and you are on a trial.</Text>
  <Text><Button size="sm" variant="primary" href={accountsSignUpUrl()}>Sign up</Button></Text>
{/if}
