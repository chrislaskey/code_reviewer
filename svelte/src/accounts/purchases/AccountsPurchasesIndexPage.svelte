<!-- The signed-in person's one-time purchases, newest first, at
     /accounts/purchases. See docs/design/purchasing.md, P5. -->
<script>
  import { accountsPurchasesList } from "../../lib/core/index.js";
  import { loaded } from "../../lib/loaded.svelte.js";
  import { accountsProductsIndexUrl, accountsPurchasesShowUrl } from "../../lib/routes.js";
  import { moneyFormat } from "../money.js";
  import { ErrorMessage, Heading, Link, Page, Text } from "../../lib/components/index.js";

  const purchases = loaded(accountsPurchasesList);
</script>

<Page>
  <Heading level={1}>Purchases</Heading>

  <ErrorMessage message={purchases.error} />
  {#if purchases.value?.length === 0}
    <Text tone="muted">Nothing bought yet. <Link href={accountsProductsIndexUrl()}>See what is for sale</Link></Text>
  {/if}

  <ul class="flex flex-col gap-2">
    {#each purchases.value ?? [] as purchase (purchase.id)}
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
