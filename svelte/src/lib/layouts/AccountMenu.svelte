<!-- The person in the app's header. Logged in, their avatar, or their
     initials, opens a menu: who they are, by their email, "On this device
     only" for a local user, or "Guest" for a trial user, whose name is a
     made-up one, with what that means and Sign up, AccountsGuestNotice,
     which a dot on the avatar points to; links to their profile and their
     account's settings; Switch account in the app, to the device's
     accounts, where a double tap on the avatar switches to the next
     signed-in account at once, on the page last visited in it; and Log out. A local user and a guest have no log-out,
     having nothing to log back in with, and no account settings. A
     visitor sees Log in and Sign up on the web, and Log in in the app, to
     the device's accounts. -->
<script>
  import { t } from "ttag";
  import AccountsGuestNotice from "../AccountsGuestNotice.svelte";
  import HeaderMenu from "./HeaderMenu.svelte";
  import { accountsSignOut, deviceAccountsSwitchNext, hasDeviceAccounts } from "../core/index.js";
  import { errorMessageOf } from "../error_message.js";
  import { currentUser, currentUserForget } from "../current_user.svelte.js";
  import { navigateToUrl } from "../router.js";
  import { pageAfterSignInUrl } from "../pages.js";
  import {
    accountsIndexUrl,
    accountsCurrentShowUrl,
    accountsSignInUrl,
    profileEditUrl,
  } from "../routes.js";
  import { Avatar, Button, ErrorMessage, Icon, Link } from "../components/index.js";


  let isGuest = $derived(currentUser.value?.kind === "trial");
  let busy = $state(false);
  let error = $state("");

  async function signOut() {
    error = "";
    busy = true;
    try {
      await accountsSignOut();
      currentUserForget();
      await navigateToUrl(hasDeviceAccounts ? accountsIndexUrl() : accountsSignInUrl());
    } catch (problem) {
      error = errorMessageOf(problem);
    } finally {
      busy = false;
    }
  }

  async function switchToNextAccount() {
    if (busy) return;
    error = "";
    busy = true;
    try {
      const switched = await deviceAccountsSwitchNext();
      if (switched) {
        currentUserForget();
        await navigateToUrl(switched.last_visited_page ?? pageAfterSignInUrl());
      }
    } catch (problem) {
      error = errorMessageOf(problem);
    } finally {
      busy = false;
    }
  }
</script>

{#if currentUser.value}
  <HeaderMenu label={isGuest ? t`Account, as a guest` : t`Account`} buttonClass="btn-circle"
    ondoubletap={hasDeviceAccounts ? switchToNextAccount : undefined}
  >
    {#snippet button()}
      <span class="relative inline-flex">
        <Avatar user={currentUser.value} />
        {#if isGuest}
          <span class="bg-warning ring-base-100 absolute -top-0.5 -right-0.5 size-2.5 rounded-full ring-2" data-guest-dot></span>
        {/if}
      </span>
    {/snippet}
    {#snippet children(close)}
      <div class="flex flex-col px-3 pt-2 pb-1">
        {#if isGuest}
          <span class="mb-1 font-semibold">{t`Guest`}</span>
          {#if currentUser.value.trial_expires_at}
            <AccountsGuestNotice trialExpiresAt={currentUser.value.trial_expires_at} onsignup={close} />
          {/if}
        {:else}
          <span class="truncate font-semibold">{currentUser.value.name}</span>
          <span class="muted truncate">{currentUser.value.kind === "local" ? t`On this device only` : currentUser.value.email}</span>
        {/if}
      </div>
      <ul class="menu w-full p-0">
        <li>
          <a href={profileEditUrl()} onclick={close}><Icon name="hero-user-circle" />{t`Profile`}</a>
        </li>
        {#if currentUser.value.kind === "full"}
          <li>
            <a href={accountsCurrentShowUrl()} onclick={close}><Icon name="hero-cog-6-tooth" />{t`Account settings`}</a>
          </li>
        {/if}
        {#if hasDeviceAccounts}
          <li>
            <a href={accountsIndexUrl()} onclick={close}><Icon name="hero-users" />{t`Switch account`}</a>
          </li>
        {/if}
        {#if currentUser.value.kind === "full"}
          <li>
            <button type="button" onclick={signOut} disabled={busy}>
              <Icon name="hero-arrow-right-start-on-rectangle" />{t`Log out`}
            </button>
          </li>
        {/if}
      </ul>
      <ErrorMessage message={error} />
    {/snippet}
  </HeaderMenu>
{:else if hasDeviceAccounts}
  <Button variant="primary" size="sm" href={accountsIndexUrl()}>{t`Log in`}</Button>
{:else}
  <Link href={accountsSignInUrl()}>{t`Log in`}</Link>
  <Button variant="primary" size="sm" href={accountsSignInUrl()}>{t`Sign up`}</Button>
{/if}
