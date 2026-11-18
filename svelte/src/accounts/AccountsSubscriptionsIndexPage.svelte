<!-- The signed-in person's plans, at /accounts/subscriptions: the
     current one and the ended ones, newest first. See
     docs/design/purchasing.md, P6. -->
<script>
  import { accountsSubscriptionsList } from "../lib/core/index.js";
  import { accountsProductsIndexUrl, accountsSubscriptionsShowUrl } from "../lib/routes.js";
  import { priceFormat } from "./money.js";
  import { subscriptionStatusSay } from "./subscription_status.js";
  import { ErrorMessage, Heading, Link, Text } from "../lib/components/index.js";
  import Page from "../lib/Page.svelte";

  let subscriptions = $state([]);
  let loaded = $state(false);
  let error = $state("");

  accountsSubscriptionsList().then(
    (list) => {
      subscriptions = list;
      loaded = true;
    },
    (problem) => (error = String(problem)),
  );
</script>

<Page>
  <Heading level={1}>Plans</Heading>

  <ErrorMessage message={error} />
  {#if loaded && subscriptions.length === 0}
    <Text tone="muted">No plan yet. <Link href={accountsProductsIndexUrl()}>See the plans</Link></Text>
  {/if}

  <ul class="flex flex-col gap-2">
    {#each subscriptions as subscription (subscription.id)}
      <li>
        <Link href={accountsSubscriptionsShowUrl(subscription.id)}>{subscription.product_name}</Link>
        <Text as="span" tone="muted">· {priceFormat(subscription.price)} · {subscriptionStatusSay(subscription.status)}</Text>
      </li>
    {/each}
  </ul>
</Page>
