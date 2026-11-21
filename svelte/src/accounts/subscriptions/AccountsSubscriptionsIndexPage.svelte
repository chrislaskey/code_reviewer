<!-- The signed-in person's plans, at /accounts/subscriptions: the
     current one and the ended ones, newest first. See
     docs/design/purchasing.md, P6. -->
<script>
  import { accountsSubscriptionsList } from "../../lib/core/index.js";
  import { loaded } from "../../lib/loaded.svelte.js";
  import { accountsProductsIndexUrl, accountsSubscriptionsShowUrl } from "../../lib/routes.js";
  import { priceFormat } from "../money.js";
  import { subscriptionStatusSay } from "./subscription_status.js";
  import { ErrorMessage, Heading, Link, Page, Text } from "../../lib/components/index.js";

  const subscriptions = loaded(accountsSubscriptionsList);
</script>

<Page>
  <Heading level={1}>Plans</Heading>

  <ErrorMessage message={subscriptions.error} />
  {#if subscriptions.value?.length === 0}
    <Text tone="muted">No plan yet. <Link href={accountsProductsIndexUrl()}>See the plans</Link></Text>
  {/if}

  <ul class="flex flex-col gap-2">
    {#each subscriptions.value ?? [] as subscription (subscription.id)}
      <li>
        <Link href={accountsSubscriptionsShowUrl(subscription.id)}>{subscription.product_name}</Link>
        <Text as="span" tone="muted">· {priceFormat(subscription.price)} · {subscriptionStatusSay(subscription.status)}</Text>
      </li>
    {/each}
  </ul>
</Page>
