<!-- The person in the app's header. Signed in, their avatar, or their
     initials, opens a menu: who they are, by their email, "On this device
     only" for a local user, or "Trial" with Sign up for a trial user,
     whose name is a made-up one; links to their profile and their
     account's settings; Switch account
     in the app, to the device's accounts; and Sign out. A local user and a
     trial user have no sign-out, having nothing to sign back in with, and
     no account settings. A visitor sees Sign in and Sign up on the web,
     and Sign in in the app, to the device's accounts. -->
<script>
  import { t } from "ttag";
  import HeaderMenu from "./HeaderMenu.svelte";
  import { accountsSignOut, hasDeviceAccounts } from "../core/index.js";
  import { errorMessageOf } from "../error_message.js";
  import { currentUser, currentUserForget } from "../current_user.svelte.js";
  import { navigateToUrl } from "../router.js";
  import {
    accountsIndexUrl,
    accountsCurrentShowUrl,
    accountsSignInUrl,
    accountsSignUpUrl,
    profileEditUrl,
  } from "../routes.js";
  import { Button, ErrorMessage, Icon, Link } from "../components/index.js";


  let busy = $state(false);
  let error = $state("");

  let initials = $derived(
    (currentUser.value?.name ?? "")
      .split(/\s+/)
      .filter(Boolean)
      .slice(0, 2)
      .map((word) => word[0].toUpperCase())
      .join(""),
  );

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
</script>

{#if currentUser.value}
  <HeaderMenu label={t`Account`} buttonClass="btn-circle">
    {#snippet button()}
      {#if currentUser.value.avatar_url}
        <img src={currentUser.value.avatar_url} alt="" class="size-7 rounded-full object-cover" />
      {:else if initials}
        <span class="avatar-initials">{initials}</span>
      {:else}
        <Icon name="hero-user-circle" class="size-7" />
      {/if}
    {/snippet}
    {#snippet children(close)}
      <div class="flex flex-col px-3 pt-2 pb-1">
        {#if currentUser.value.kind === "trial"}
          <span class="font-semibold">{t`Trial`}</span>
          <span class="muted">{t`Sign up to keep what you make.`}</span>
        {:else}
          <span class="truncate font-semibold">{currentUser.value.name}</span>
          <span class="muted truncate">{currentUser.value.kind === "local" ? t`On this device only` : currentUser.value.email}</span>
        {/if}
      </div>
      {#if currentUser.value.kind === "trial"}
        <div class="px-3 py-1">
          <Button variant="primary" size="sm" class="w-full" href={accountsSignUpUrl("")} onclick={close}>
            {t`Sign up`}
          </Button>
        </div>
      {/if}
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
              <Icon name="hero-arrow-right-start-on-rectangle" />{t`Sign out`}
            </button>
          </li>
        {/if}
      </ul>
      <ErrorMessage message={error} />
    {/snippet}
  </HeaderMenu>
{:else if hasDeviceAccounts}
  <Button variant="primary" size="sm" href={accountsIndexUrl()}>{t`Sign in`}</Button>
{:else}
  <Link href={accountsSignInUrl()}>{t`Sign in`}</Link>
  <Button variant="primary" size="sm" href={accountsSignUpUrl()}>{t`Sign up`}</Button>
{/if}
