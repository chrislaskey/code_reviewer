<!-- The signed-in person's account, at /accounts/settings: their email,
     the way to their password, and what they can buy, see
     docs/design/purchasing.md. -->
<script>
  import PublicPagesLinks from "../lib/PublicPagesLinks.svelte";
  import AccountsNeeded from "../lib/AccountsNeeded.svelte";
  import { currentUserLoad } from "../lib/current_user.js";
  import {
    accountsDeletionsNewUrl,
    accountsPasswordEditUrl,
    accountsProductsIndexUrl,
    accountsPurchasesIndexUrl,
    accountsSubscriptionsIndexUrl,
  } from "../lib/routes.js";
  import { Heading, Link, Page, Text } from "../lib/components/index.js";

  /** @type {import("../lib/current_user.js").User | null} */
  let currentUser = $state(null);

  currentUserLoad().then(
    (user) => (currentUser = user),
    () => (currentUser = null),
  );
</script>

<Page>
  <Heading level={1}>Account</Heading>
  {#if currentUser && currentUser.kind !== "full"}
    <AccountsNeeded kind={currentUser.kind} />
    {#if currentUser.kind === "trial"}
      <Text><Link href={accountsDeletionsNewUrl()}>Delete my trial</Link></Text>
    {/if}
  {:else}
    {#if currentUser}
      <Text>{currentUser.email}</Text>
    {/if}
    <Text><Link href={accountsPasswordEditUrl()}>Password</Link></Text>
    {#if currentUser?.plan}
      <Text>Your plan: {currentUser.plan.product_name}</Text>
    {/if}
    <Text><Link href={accountsSubscriptionsIndexUrl()}>Plans</Link></Text>
    <Text><Link href={accountsProductsIndexUrl()}>Plans and products for sale</Link></Text>
    <Text><Link href={accountsPurchasesIndexUrl()}>Purchases</Link></Text>
    <Text><Link tone="danger" href={accountsDeletionsNewUrl()}>Delete my account</Link></Text>
  {/if}
  <Text tone="muted"><PublicPagesLinks /></Text>
</Page>
