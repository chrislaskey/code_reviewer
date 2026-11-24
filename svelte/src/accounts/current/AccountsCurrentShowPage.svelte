<!-- The signed-in person's account, at /accounts/current: their email,
     the way to their password, and what they can buy, see
     docs/design/purchasing.md; nothing to buy on an account of a server
     other than the app's own, see docs/design/multi-server.md, or of one
     whose settings turn purchasing off. -->
<script>
  import { t } from "ttag";
  import PublicPagesLinks from "../../lib/PublicPagesLinks.svelte";
  import AccountsNeeded from "../../lib/AccountsNeeded.svelte";
  import { currentUser } from "../../lib/current_user.svelte.js";
  import { currentServer, currentServerLoad } from "../../lib/current_server.svelte.js";
  import { errorMessageOf } from "../../lib/error_message.js";
  import {
    accountsDeletionsNewUrl,
    accountsCurrentPasswordEditUrl,
    accountsProductsIndexUrl,
    accountsPurchasesIndexUrl,
    accountsSubscriptionsIndexUrl,
  } from "../../lib/routes.js";
  import { ErrorMessage, Heading, Link, Page, Text } from "../../lib/components/index.js";

  let error = $state("");
  currentServerLoad().catch((problem) => (error = errorMessageOf(problem)));
  /** Whether this account buys anywhere: on the app's own server alone, when it sells. */
  let sells = $derived(
    (currentServer.value?.default ?? false) && (currentServer.value?.settings.features.accounts_purchasing ?? true),
  );
</script>

<Page>
  <Heading level={1}>{t`Account`}</Heading>
  <ErrorMessage message={error} />
  {#if currentUser.value && currentUser.value.kind !== "full"}
    <AccountsNeeded kind={currentUser.value.kind} />
    {#if currentUser.value.kind === "trial"}
      <Text><Link href={accountsDeletionsNewUrl()}>{t`Delete what I made as a guest`}</Link></Text>
    {/if}
  {:else}
    {#if currentUser.value}
      <Text>{currentUser.value.email}</Text>
    {/if}
    <Text><Link href={accountsCurrentPasswordEditUrl()}>{t`Password`}</Link></Text>
    {#if sells}
      {#if currentUser.value?.plan}
        <Text>{t`Your plan: ${currentUser.value.plan.product_name}`}</Text>
      {/if}
      <Text><Link href={accountsSubscriptionsIndexUrl()}>{t`Plans`}</Link></Text>
      <Text><Link href={accountsProductsIndexUrl()}>{t`Plans and products for sale`}</Link></Text>
      <Text><Link href={accountsPurchasesIndexUrl()}>{t`Purchases`}</Link></Text>
    {/if}
    <Text><Link tone="danger" href={accountsDeletionsNewUrl()}>{t`Delete my account`}</Link></Text>
  {/if}
  <Text tone="muted"><PublicPagesLinks /></Text>
</Page>
