<!-- The router's layout, around every page: the header (AppHeader) and
     the navigation (AppNav), which outlive the pages so they never redraw
     as they swap, and the page below the header beside the navigation.
     Someone whose plan's payment failed sees that banner above every page;
     a guest's notice is in the account menu instead, see AccountMenu. -->
<script>
  import AccountsPaymentFailedBanner from "./AccountsPaymentFailedBanner.svelte";
  import AppHeader from "./AppHeader.svelte";
  import AppNav from "./AppNav.svelte";
  import RemountPageOnNavigation from "./RemountPageOnNavigation.svelte";
  import { currentUser, currentUserLoad } from "../current_user.svelte.js";
  import { currentPathname } from "../router.js";

  let { children } = $props();

  // Asked again on every page, so a sign-in or a sign-out shows here; a
  // failure leaves nobody shown, and the page that needs someone says why.
  $effect(() => {
    currentPathname();
    currentUserLoad().catch(() => {});
  });
</script>

<AppHeader />
<AppNav />
<div class="app-layout">
  {#if currentUser.value?.payment_failed_subscription_id}
    <div class="app-banner">
      <AccountsPaymentFailedBanner subscriptionId={currentUser.value.payment_failed_subscription_id} />
    </div>
  {/if}
  <RemountPageOnNavigation>
    {@render children()}
  </RemountPageOnNavigation>
</div>
