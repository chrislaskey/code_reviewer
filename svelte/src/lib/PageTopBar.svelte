<!-- Above every page, what the navigation (AppNav) has no place for yet:
     the current community to switch, a link to the profile, who is
     signed in, by their email, a link to their account, with Sign out,
     or Sign in and Sign up on the web and Accounts in the app, and inside
     the app the sync status. A local user has no email and no sign-out:
     "On this device only", to their profile. A trial user sees the
     trial's banner above every page, and has no sign-out either, having
     nothing to sign back in with. -->
<script>
  import AccountsPaymentFailedBanner from "../accounts/AccountsPaymentFailedBanner.svelte";
  import AccountsTrialBanner from "../accounts/AccountsTrialBanner.svelte";
  import CurrentCommunityMenu from "./CurrentCommunityMenu.svelte";
  import SyncStatus from "./SyncStatus.svelte";
  import { accountsSignOut, hasDeviceAccounts } from "./core/index.js";
  import { currentUserForget, currentUserLoad } from "./current_user.js";
  import { navigateToUrl } from "./router.js";
  import {
    accountsIndexUrl,
    accountsSettingsUrl,
    accountsSignInUrl,
    accountsSignUpUrl,
    profileEditUrl,
  } from "./routes.js";
  import { Button, ErrorMessage, Link } from "./components/index.js";

  /** @type {import("./current_user.js").User | null} */
  let currentUser = $state(null);
  let busy = $state(false);
  let error = $state("");

  currentUserLoad().then(
    (user) => (currentUser = user),
    () => (currentUser = null),
  );

  async function signOut() {
    error = "";
    busy = true;
    try {
      await accountsSignOut();
      currentUserForget();
      await navigateToUrl(hasDeviceAccounts ? accountsIndexUrl() : accountsSignInUrl());
    } catch (problem) {
      error = String(problem);
    } finally {
      busy = false;
    }
  }
</script>

{#if currentUser?.kind === "trial" && currentUser.trial_expires_at}
  <AccountsTrialBanner trialExpiresAt={currentUser.trial_expires_at} />
{/if}
{#if currentUser?.payment_failed_subscription_id}
  <AccountsPaymentFailedBanner subscriptionId={currentUser.payment_failed_subscription_id} />
{/if}
<div class="flex flex-wrap items-center gap-4">
  {#if currentUser}
    <CurrentCommunityMenu />
    <Link href={profileEditUrl()}>Profile</Link>
  {/if}
  <span class="grow"></span>
  {#if currentUser}
    {#if currentUser.kind === "local"}
      <Link size="sm" href={profileEditUrl()}>On this device only</Link>
    {:else if currentUser.kind === "trial"}
      <Link size="sm" href={accountsSignUpUrl("")}>Trial</Link>
    {:else}
      <Link size="sm" href={accountsSettingsUrl()}>{currentUser.email}</Link>
    {/if}
    {#if hasDeviceAccounts}
      <Link size="sm" href={accountsIndexUrl()}>Accounts</Link>
    {/if}
    {#if currentUser.kind === "full"}
      <Button variant="ghost" size="sm" onclick={signOut} disabled={busy}>Sign out</Button>
    {/if}
  {:else if hasDeviceAccounts}
    <Link href={accountsIndexUrl()}>Accounts</Link>
  {:else}
    <Link href={accountsSignInUrl()}>Sign in</Link>
    <Link href={accountsSignUpUrl()}>Sign up</Link>
  {/if}
</div>
<ErrorMessage message={error} />
{#if currentUser}
  <SyncStatus />
{/if}
