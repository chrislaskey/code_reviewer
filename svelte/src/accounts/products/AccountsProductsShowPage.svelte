<!-- One product for sale, at /accounts/products/:id: what it is, and a
     Buy button per price, which goes to Stripe's page to pay, or on a
     phone opens its store's sheet, with the store's price, the plan's
     length and the terms and privacy links Apple asks for. Someone
     with no account yet is asked to sign up; a one-time product the
     person has is Owned, unless it can be bought again, and someone on
     a plan does not buy another: they switch, in the store the plan is
     from. See docs/design/purchasing.md, P1, P2 and P4, and
     docs/design/purchasing-phones.md, M1, M4 and M5. -->
<script>
  import PublicPagesLinks from "../../lib/PublicPagesLinks.svelte";
  import AccountsNeeded from "../../lib/AccountsNeeded.svelte";
  import {
    accountsCheckoutsCreate,
    accountsProductsGet,
    accountsStoreTransactionsCreate,
    accountsSubscriptionsUpdate,
  } from "../../lib/core/index.js";
  import { loaded } from "../../lib/loaded.svelte.js";
  import { errorMessageOf } from "../../lib/error_message.js";
  import { currentUserLoad, currentUserReload } from "../../lib/current_user.js";
  import { navigateToUrl, pathParams } from "../../lib/router.js";
  import {
    accountsCheckoutsShowUrl,
    accountsProductsIndexUrl,
    accountsPurchasesShowUrl,
    accountsSubscriptionsShowUrl,
  } from "../../lib/routes.js";
  import { priceFormat } from "../money.js";
  import { storeManagedWhere, storeName, storePriceFormat } from "../stores.js";
  import { Button, ErrorMessage, Heading, Link, Page, Text } from "../../lib/components/index.js";

  const id = pathParams().id;
  const shown = loaded(() => accountsProductsGet(id));
  const user = loaded(currentUserLoad);
  let product = $derived(shown.value);
  let currentUser = $derived(user.value ?? null);
  let buying = $state(false);
  let pending = $state(false);
  let error = $state("");
  /** Where this app sells: Stripe, or the phone's store. */
  const store = $derived(product?.store ?? "stripe");

  /** @param {string} priceId */
  async function planSwitch(priceId) {
    const plan = currentUser?.plan;
    if (!plan) return;
    buying = true;
    error = "";
    try {
      await accountsSubscriptionsUpdate(plan.subscription_id, { action: "switch", price_id: priceId });
      await currentUserReload();
      await navigateToUrl(accountsSubscriptionsShowUrl(plan.subscription_id));
    } catch (problem) {
      error = errorMessageOf(problem);
      buying = false;
    }
  }

  /** @param {string} priceId */
  async function buy(priceId) {
    if (store !== "stripe") return storeBuy(priceId);
    buying = true;
    error = "";
    try {
      const { checkout, url } = await accountsCheckoutsCreate(priceId);
      // In the app Stripe's page opens in the browser, and the checkout's page waits
      if (url) window.location.assign(url);
      else await navigateToUrl(accountsCheckoutsShowUrl(checkout.id));
    } catch (problem) {
      error = errorMessageOf(problem);
      buying = false;
    }
  }

  /**
   * The store's sheet; a switch is buying the other plan there, and the
   * store decides when it starts.
   * @param {string} priceId
   */
  async function storeBuy(priceId) {
    buying = true;
    pending = false;
    error = "";
    try {
      const bought = await accountsStoreTransactionsCreate(priceId);
      if (bought.pending) pending = true;
      if (bought.purchase || bought.subscription) await currentUserReload();
      if (bought.purchase) return navigateToUrl(accountsPurchasesShowUrl(bought.purchase.id));
      if (bought.subscription) return navigateToUrl(accountsSubscriptionsShowUrl(bought.subscription.id));
    } catch (problem) {
      error = errorMessageOf(problem);
    }
    buying = false;
  }
</script>

<Page>

  <ErrorMessage message={error || shown.error || user.error} />
  {#if product}
    <Heading level={1}>{product.name}</Heading>
    {#if product.description}
      <Text>{product.description}</Text>
    {/if}
    {#if product.free_days}
      <Text tone="muted">The first {product.free_days} days are free.</Text>
    {/if}
    {#if currentUser && currentUser.kind !== "full"}
      <ul>
        {#each product.prices as price (price.id)}
          <li>{storePriceFormat(price, priceFormat)}</li>
        {/each}
      </ul>
      <AccountsNeeded kind={currentUser.kind} />
    {:else if product.kind === "plan" && currentUser?.plan}
      {@const plan = currentUser.plan}
      <Text>You are on {plan.product_name}.</Text>
      {#if plan.store !== store}
        <Text tone="muted">It was bought in {storeName(plan.store)}: manage it {storeManagedWhere(plan.store)}.</Text>
      {:else if plan.cancel_at_period_end}
        <Text tone="muted">It ends with this period. Resume it on its page to switch.</Text>
      {:else}
        <div class="flex flex-wrap gap-2">
          {#each product.prices as price (price.id)}
            {#if price.id !== plan.price_id}
              <Button variant="primary" size="sm"
                disabled={buying}
                onclick={() => (store === "stripe" ? planSwitch(price.id) : storeBuy(price.id))}
              >
                Switch · {storePriceFormat(price, priceFormat)}
              </Button>
            {/if}
          {/each}
        </div>
        {#if store === "stripe"}
          <Text tone="muted">A dearer plan, or yearly, starts now with the difference charged; a cheaper one, or monthly, starts when this period ends.</Text>
        {:else}
          <Text tone="muted">{storeName(store)} decides when the new plan starts: an upgrade at once, a downgrade at the renewal.</Text>
        {/if}
      {/if}
      <Text><Link href={accountsSubscriptionsShowUrl(plan.subscription_id)}>See your plan</Link></Text>
    {:else if product.owned && !product.is_repeatable}
      <Text>Owned</Text>
    {:else}
      {#if product.owned}
        <Text tone="muted">You have this. You can buy it again.</Text>
      {/if}
      <div class="flex flex-wrap gap-2">
        {#each product.prices as price (price.id)}
          <Button variant="primary" size="sm" disabled={buying} onclick={() => buy(price.id)}>
            Buy · {storePriceFormat(price, priceFormat)}
          </Button>
        {/each}
      </div>
      {#if pending}
        <Text tone="muted">Waiting for approval. It appears here once {storeName(store)} confirms it.</Text>
      {/if}
    {/if}
    {#if store !== "stripe" && product.kind === "plan"}
      <Text tone="muted" size="sm">
        {product.name}: an auto-renewing subscription, renewed each {product.prices
          .map((price) => price.store_period ?? price.interval)
          .filter(Boolean)
          .join(" or ")} at the price shown until you cancel it in {storeName(store)}.
        By buying you accept the <PublicPagesLinks />.
      </Text>
    {/if}
  {/if}

  <Link href={accountsProductsIndexUrl()}>Back to plans and products</Link>
</Page>
