<!-- What is for sale, at /accounts/products: plans, then one-time
     products, each with its prices and a link to its page, where it is
     bought; on a phone with its store's prices, and on the App Store
     with Restore purchases, as Apple asks. See docs/design/purchasing.md,
     P1, and docs/design/purchasing-phones.md, M2. -->
<script>
  import { accountsProductsList, accountsStoreTransactionsRestore } from "../../lib/core/index.js";
  import { loaded } from "../../lib/loaded.svelte.js";
  import { errorMessageOf } from "../../lib/error_message.js";
  import { currentUserReload } from "../../lib/current_user.svelte.js";
  import { accountsProductsShowUrl } from "../../lib/routes.js";
  import { priceFormat } from "../money.js";
  import { storePriceFormat } from "../stores.js";
  import { Button, ErrorMessage, Heading, Link, Page, Text } from "../../lib/components/index.js";

  const shown = loaded(accountsProductsList);
  let products = $derived(shown.value ?? []);
  let error = $state("");
  let restoring = $state(false);
  let restored = $state("");
  const store = $derived(products[0]?.store ?? "stripe");

  async function restore() {
    restoring = true;
    error = "";
    restored = "";
    try {
      const { recorded, refused } = await accountsStoreTransactionsRestore();
      await currentUserReload();
      restored = recorded === 1 ? "1 purchase restored." : `${recorded} purchases restored.`;
      if (refused.length > 0) error = refused.join(" ");
    } catch (problem) {
      error = errorMessageOf(problem);
    } finally {
      restoring = false;
    }
  }
</script>

<Page>
  <Heading level={1}>Plans and products</Heading>

  <ErrorMessage message={error || shown.error} />
  {#if shown.value?.length === 0}
    <Text tone="muted">Nothing is for sale yet.</Text>
  {/if}

  <ul class="flex flex-col gap-4">
    {#each products as product (product.id)}
      <li>
        <Heading><Link href={accountsProductsShowUrl(product.id)}>{product.name}</Link></Heading>
        {#if product.description}
          <Text>{product.description}</Text>
        {/if}
        <Text tone="muted">
          {product.kind === "plan" ? "Plan" : "One-time"} ·
          {product.prices.map((price) => storePriceFormat(price, priceFormat)).join(" or ")}
        </Text>
      </li>
    {/each}
  </ul>

  {#if store === "app_store"}
    <div class="mt-6 flex flex-wrap items-center gap-3">
      <Button size="sm" disabled={restoring} onclick={restore}>Restore purchases</Button>
      {#if restored}
        <Text as="span" tone="muted">{restored}</Text>
      {/if}
    </div>
  {/if}
</Page>
