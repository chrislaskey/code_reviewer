<!-- One of the signed-in person's purchases, at /accounts/purchases/:id.
     The receipt is the email Stripe sent. See docs/design/purchasing.md, P5. -->
<script>
  import { accountsPurchasesGet } from "../../lib/core/index.js";
  import { pathParams } from "../../lib/router.js";
  import { accountsPurchasesIndexUrl } from "../../lib/routes.js";
  import { moneyFormat } from "../money.js";
  import { ErrorMessage, Heading, Link, Page, Text } from "../../lib/components/index.js";

  const id = pathParams().id;
  let purchase = $state();
  let error = $state("");

  accountsPurchasesGet(id).then(
    (shown) => (purchase = shown),
    (problem) => (error = String(problem)),
  );
</script>

<Page>

  <ErrorMessage message={error} />
  {#if purchase}
    <Heading level={1}>{purchase.product_name}</Heading>
    <Text>{moneyFormat(purchase.amount, purchase.currency)}, tax included</Text>
    <Text tone="muted">Bought {new Date(purchase.inserted_at).toLocaleString()}</Text>
    {#if purchase.refunded_at}
      <Text>Refunded {new Date(purchase.refunded_at).toLocaleString()}</Text>
    {/if}
    <Text tone="muted">The receipt is in the email sent when you paid.</Text>
  {/if}

  <Link href={accountsPurchasesIndexUrl()}>Back to purchases</Link>
</Page>
