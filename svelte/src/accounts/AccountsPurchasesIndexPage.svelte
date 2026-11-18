<!-- The signed-in person's one-time purchases, newest first, at
     /accounts/purchases. See docs/design/purchasing.md, P5. -->
<script>
  import { accountsPurchasesList } from "../lib/core/index.js";
  import { accountsProductsIndexUrl, accountsPurchasesShowUrl } from "../lib/routes.js";
  import { moneyFormat } from "./money.js";
  import { ErrorMessage, Heading, Link, Text } from "../lib/components/index.js";
  import Page from "../lib/Page.svelte";

  let purchases = $state([]);
  let loaded = $state(false);
  let error = $state("");

  accountsPurchasesList().then(
    (list) => {
      purchases = list;
      loaded = true;
    },
    (problem) => (error = String(problem)),
  );
</script>

<Page>
  <Heading level={1}>Purchases</Heading>

  <ErrorMessage message={error} />
  {#if loaded && purchases.length === 0}
    <Text tone="muted">Nothing bought yet. <Link href={accountsProductsIndexUrl()}>See what is for sale</Link></Text>
  {/if}

  <ul class="flex flex-col gap-2">
    {#each purchases as purchase (purchase.id)}
      <li>
        <Link href={accountsPurchasesShowUrl(purchase.id)}>{purchase.product_name}</Link>
        <Text as="span" tone="muted">
          · {moneyFormat(purchase.amount, purchase.currency)} · {new Date(purchase.inserted_at).toLocaleDateString()}
          {#if purchase.refunded_at}· Refunded{/if}
        </Text>
      </li>
    {/each}
  </ul>
</Page>
