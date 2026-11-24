<!-- Shown in place of what needs an account on the server, to someone
     who has none yet: a local user, in the app, or a trial user, on the
     web. See docs/design/accounts-local-and-trial-users.md. -->
<script>
  import { t } from "ttag";
  import { accountsIndexUrl, accountsSignInUrl } from "./routes.js";
  import { Button, Text } from "./components/index.js";

  /**
   * @typedef {Object} Props
   * @property {"local" | "trial"} kind
   */

  /** @type {Props} */
  let { kind } = $props();
</script>

{#if kind === "local"}
  <Text>{t`This is online only, and your account is only on this device.`}</Text>
  <Text><Button size="sm" variant="primary" href={accountsIndexUrl()}>{t`Sign up or log in`}</Button></Text>
{:else}
  <Text>{t`This needs an account, and you are a guest.`}</Text>
  <Text><Button size="sm" variant="primary" href={accountsSignInUrl()}>{t`Sign up`}</Button></Text>
{/if}
