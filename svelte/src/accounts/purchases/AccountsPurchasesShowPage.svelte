<!-- One of the signed-in person's purchases, at /accounts/purchases/:id.
     The receipt is the email Stripe sent. See docs/design/purchasing.md, P5. -->
<script>
  import { t } from "ttag";
  import { accountsPurchasesGet } from "../../lib/core/index.js";
  import { loaded } from "../../lib/loaded.svelte.js";
  import { pathParams } from "../../lib/router.js";
  import { accountsPurchasesIndexUrl } from "../../lib/routes.js";
  import { moneyFormat } from "../money.js";
  import { dateTimeFormat } from "../../lib/formats.js";
  import { ErrorMessage, Heading, Link, Page, Text } from "../../lib/components/index.js";

  const id = pathParams().id;
  const shown = loaded(() => accountsPurchasesGet(id));
</script>

<Page>
  <ErrorMessage message={shown.error} />
  {#if shown.value}
    {@const purchase = shown.value}
    <Heading level={1}>{purchase.product_name}</Heading>
    {@const amount = moneyFormat(purchase.amount, purchase.currency)}
    {@const boughtAt = dateTimeFormat(purchase.inserted_at)}
    <Text>{t`${amount}, tax included`}</Text>
    <Text tone="muted">{t`Bought ${boughtAt}`}</Text>
    {#if purchase.refunded_at}
      {@const refundedAt = dateTimeFormat(purchase.refunded_at)}
      <Text>{t`Refunded ${refundedAt}`}</Text>
    {/if}
    <Text tone="muted">{t`The receipt is in the email sent when you paid.`}</Text>
  {/if}

  <Link href={accountsPurchasesIndexUrl()}>{t`Back to purchases`}</Link>
</Page>
