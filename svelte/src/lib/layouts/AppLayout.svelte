<!-- The router's layout, around every page: the header (AppHeader) and
     the navigation (AppNav), which outlive the pages so they never redraw
     as they swap, and the page below the header beside the navigation. A
     trial user sees the trial's banner above every page, and someone
     whose plan's payment failed sees that banner. -->
<script>
  import AccountsPaymentFailedBanner from "./AccountsPaymentFailedBanner.svelte";
  import AccountsTrialBanner from "./AccountsTrialBanner.svelte";
  import AppHeader from "./AppHeader.svelte";
  import AppNav from "./AppNav.svelte";
  import RemountPageOnNavigation from "./RemountPageOnNavigation.svelte";
  import { currentUserLoad } from "../current_user.js";
  import { currentPathname } from "../router.js";

  let { children } = $props();

  /** @type {import("../current_user.js").User | null} */
  let currentUser = $state(null);

  // Asked again on every page, so a sign-in or a sign-out shows here.
  $effect(() => {
    currentPathname();
    currentUserLoad().then(
      (user) => (currentUser = user),
      () => (currentUser = null),
    );
  });
</script>

<AppHeader {currentUser} />
<AppNav {currentUser} />
<div class="app-layout">
  {#if currentUser?.kind === "trial" && currentUser.trial_expires_at}
    <div class="app-banner">
      <AccountsTrialBanner trialExpiresAt={currentUser.trial_expires_at} />
    </div>
  {/if}
  {#if currentUser?.payment_failed_subscription_id}
    <div class="app-banner">
      <AccountsPaymentFailedBanner subscriptionId={currentUser.payment_failed_subscription_id} />
    </div>
  {/if}
  <RemountPageOnNavigation>
    {@render children()}
  </RemountPageOnNavigation>
</div>
