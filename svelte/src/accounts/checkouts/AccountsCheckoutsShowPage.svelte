<!-- Where Stripe's page returns to, at /accounts/checkouts/:id: the
     checkout as Stripe has it now. A payment that settles later shows
     Processing, and the page asks again every few seconds until it is
     paid or failed. In the app, Stripe's page is in the browser, and the
     page asks again when the app has the focus. See
     docs/design/purchasing.md, P3. -->
<script>
  import { accountsCheckoutsGet, hasDeviceAccounts } from "../../lib/core/index.js";
  import { errorMessageOf } from "../../lib/error_message.js";
  import { currentUserReload } from "../../lib/current_user.js";
  import { pathParams } from "../../lib/router.js";
  import {
    accountsProductsShowUrl,
    accountsPurchasesShowUrl,
    accountsSubscriptionsIndexUrl,
  } from "../../lib/routes.js";
  import { ErrorMessage, Heading, Link, Page, Text } from "../../lib/components/index.js";

  const ASK_AGAIN_MS = 3000;
  const ASK_AT_MOST = 40;

  const id = pathParams().id;
  let checkout = $state();
  let error = $state("");
  let asked = 0;
  /** @type {ReturnType<typeof setTimeout> | undefined} */
  let timer;

  async function load() {
    try {
      checkout = await accountsCheckoutsGet(id);
      asked += 1;
      const pending = checkout.status === "open" || checkout.status === "processing";
      if (pending && asked < ASK_AT_MOST) timer = setTimeout(load, ASK_AGAIN_MS);
      if (checkout.status === "paid") await currentUserReload();
    } catch (problem) {
      error = errorMessageOf(problem);
    }
  }

  // Back from paying in the browser, in the app: ask again
  function onFocus() {
    clearTimeout(timer);
    asked = 0;
    load();
  }

  $effect(() => () => clearTimeout(timer));
  load();
</script>

<svelte:window onfocus={onFocus} />

<Page>
  <Heading level={1}>Checkout</Heading>

  <ErrorMessage message={error} />
  {#if checkout}
    {#if checkout.status === "paid"}
      <Text>Thank you. {checkout.product_name} is yours.</Text>
      {#if checkout.purchase_id}
        <Text><Link href={accountsPurchasesShowUrl(checkout.purchase_id)}>See the purchase</Link></Text>
      {:else if checkout.kind === "plan"}
        <Text><Link href={accountsSubscriptionsIndexUrl()}>See your plan</Link></Text>
      {/if}
    {:else if checkout.status === "processing"}
      <Text>Your payment for {checkout.product_name} is processing. This page updates when it is done.</Text>
    {:else if checkout.status === "open" && hasDeviceAccounts}
      <Text>Finish paying for {checkout.product_name} in your browser. This page updates when you come back.</Text>
    {:else if checkout.status === "open"}
      <Text>{checkout.product_name} is not paid yet.</Text>
      <Text><Link href={accountsProductsShowUrl(checkout.product_id)}>Back to {checkout.product_name}</Link></Text>
    {:else}
      <Text>
        {checkout.status === "failed" ? "The payment did not go through" : "This checkout expired"}, and
        nothing was charged.
      </Text>
      <Text><Link href={accountsProductsShowUrl(checkout.product_id)}>Try again</Link></Text>
    {/if}
  {/if}
</Page>
