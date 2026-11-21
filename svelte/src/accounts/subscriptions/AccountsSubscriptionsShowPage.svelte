<!-- One of the signed-in person's plans, at /accounts/subscriptions/:id:
     its price, its status, its next renewal or the day it ends, and what
     they can do with it: cancel at the end of the period, resume, undo a
     switch waiting for then, and change their card on Stripe's Customer
     Portal. Switching is on the plan's product page. A plan from a
     phone's store is managed in that store: Manage in the App Store on
     the iPhone, and where it is managed everywhere else; a Stripe plan
     seen on a phone is managed on the web. See
     docs/design/purchasing.md, P6 to P9, and
     docs/design/purchasing-phones.md, M4 and M5. -->
<script>
  import {
    accountsStoreGet,
    accountsSubscriptionsGet,
    accountsSubscriptionsManageOpen,
    accountsSubscriptionsPortalCreate,
    accountsSubscriptionsUpdate,
  } from "../../lib/core/index.js";
  import { currentUserReload } from "../../lib/current_user.js";
  import { pathParams } from "../../lib/router.js";
  import { accountsProductsShowUrl, accountsSubscriptionsIndexUrl } from "../../lib/routes.js";
  import { priceFormat } from "../money.js";
  import { storeManagedWhere, storeName } from "../stores.js";
  import { subscriptionStatusSay } from "./subscription_status.js";
  import { Button, ErrorMessage, Heading, Link, Page, Text } from "../../lib/components/index.js";

  const id = pathParams().id;
  let subscription = $state();
  let busy = $state(false);
  let inBrowser = $state(false);
  let error = $state("");
  /** Where this app sells: the plan is managed here only when it was bought here. */
  let here = $state("stripe");

  accountsStoreGet().then((store) => (here = store));

  function load() {
    accountsSubscriptionsGet(id).then(
      (shown) => (subscription = shown),
      (problem) => (error = String(problem)),
    );
  }

  // Back from Stripe's portal in the browser, in the app: ask again
  function onFocus() {
    if (!inBrowser) return;
    inBrowser = false;
    load();
    currentUserReload();
  }

  load();

  /** @param {string} at */
  const day = (at) => new Date(at).toLocaleDateString([], { dateStyle: "medium" });

  /** @param {{action: "switch_undo" | "cancel" | "resume"}} change */
  async function update(change) {
    if (change.action === "cancel" && !confirm("Cancel your plan at the end of this period?")) return;
    busy = true;
    error = "";
    try {
      subscription = await accountsSubscriptionsUpdate(id, change);
      await currentUserReload();
    } catch (problem) {
      error = String(problem);
    } finally {
      busy = false;
    }
  }

  async function manage() {
    error = "";
    try {
      await accountsSubscriptionsManageOpen();
      inBrowser = true;
    } catch (problem) {
      error = String(problem);
    }
  }

  async function portal() {
    busy = true;
    error = "";
    try {
      const { url } = await accountsSubscriptionsPortalCreate(id);
      if (url) {
        window.location.assign(url);
        return;
      }
      inBrowser = true;
    } catch (problem) {
      error = String(problem);
    }
    busy = false;
  }
</script>

<svelte:window onfocus={onFocus} />

<Page>

  <ErrorMessage message={error} />
  {#if subscription}
    {@const ended = Boolean(subscription.ended_at) || subscription.status === "canceled"}
    <Heading level={1}>{subscription.product_name}</Heading>
    <Text>{priceFormat(subscription.price)}</Text>
    <Text>{subscriptionStatusSay(subscription.status)}</Text>
    {#if subscription.ended_at}
      <Text tone="muted">Ended {day(subscription.ended_at)}</Text>
    {:else if subscription.current_period_end && subscription.cancel_at_period_end}
      <Text tone="muted">Ends {day(subscription.current_period_end)}</Text>
    {:else if subscription.current_period_end && subscription.gives_plan}
      <Text tone="muted">Renews {day(subscription.current_period_end)}</Text>
    {/if}
    {#if subscription.scheduled_price && subscription.scheduled_at}
      <Text>Changes to {priceFormat(subscription.scheduled_price)} on {day(subscription.scheduled_at)}.</Text>
    {/if}

    {#if inBrowser && subscription.store === "stripe"}
      <Text tone="muted">Stripe's page is open in your browser. This page updates when you come back.</Text>
    {/if}
    {#if !ended && subscription.store !== here}
      <Text tone="muted">Bought in {storeName(subscription.store)}: manage it {storeManagedWhere(subscription.store)}.</Text>
    {:else if !ended && subscription.store !== "stripe"}
      <div class="flex flex-wrap gap-2">
        <Button size="sm" href={accountsProductsShowUrl(subscription.product_id)}>Switch</Button>
        <Button size="sm" onclick={manage}>Manage in {storeName(subscription.store)}</Button>
      </div>
    {:else if !ended}
      <div class="flex flex-wrap gap-2">
        {#if subscription.scheduled_price}
          <Button size="sm" disabled={busy} onclick={() => update({ action: "switch_undo" })}>
            Keep {priceFormat(subscription.price)}
          </Button>
        {/if}
        {#if subscription.cancel_at_period_end}
          <Button variant="primary" size="sm" disabled={busy} onclick={() => update({ action: "resume" })}>
            Resume
          </Button>
        {:else if subscription.gives_plan}
          <Button size="sm" href={accountsProductsShowUrl(subscription.product_id)}>Switch</Button>
          <Button variant="ghost" size="sm" disabled={busy} onclick={() => update({ action: "cancel" })}>
            Cancel at the end of this period
          </Button>
        {/if}
        <Button size="sm" disabled={busy} onclick={portal}>
          {subscription.gives_plan ? "Card and invoices" : "Update your card"}
        </Button>
      </div>
    {/if}
  {/if}

  <Link href={accountsSubscriptionsIndexUrl()}>Back to plans</Link>
</Page>
