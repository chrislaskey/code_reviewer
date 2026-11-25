<!-- Deleting the signed-in person's account, at /accounts/deletions/new:
     what goes, the plan's warning by where it was bought, Send code, the
     code and Delete my account, see docs/design/accounts-delete.md. A
     trial is deleted with no code. After it, the start page: the
     public home on the web, the device's accounts in the app. A local
     user has no account; removing the device's data is the accounts
     list's. -->
<script>
  import { jt, t } from "ttag";
  import { accountsDeletionsCodeCreate, accountsDeletionsCreate, hasDeviceAccounts } from "../../lib/core/index.js";
  import { errorMessageOf } from "../../lib/error_message.js";
  import { flashMessageShow } from "../../lib/flash_messages.svelte.js";
  import { currentUser, currentUserForget, currentUserLoad } from "../../lib/current_user.svelte.js";
  import { navigateToUrl } from "../../lib/router.js";
  import { PUBLIC_HOME_PATH, accountsIndexUrl, accountsCurrentShowUrl } from "../../lib/routes.js";
  import { Button, CodeInput, ErrorMessage, Heading, Link, Page, Text, TranslatedParts } from "../../lib/components/index.js";

  /** Where each store's own subscriptions page is, to cancel a plan there. */
  const STORE_SUBSCRIPTIONS_URLS = {
    app_store: "https://apps.apple.com/account/subscriptions",
    play_store: "https://play.google.com/store/account/subscriptions",
  };

  let codeSent = $state(false);
  let code = $state("");
  let busy = $state(false);
  let error = $state("");

  currentUserLoad().catch((problem) => (error = errorMessageOf(problem)));


  /** @param {() => Promise<void>} action */
  async function doing(action) {
    busy = true;
    error = "";
    try {
      await action();
    } catch (problem) {
      error = errorMessageOf(problem);
    } finally {
      busy = false;
    }
  }

  function sendCode() {
    return doing(async () => {
      await accountsDeletionsCodeCreate();
      codeSent = true;
    });
  }

  /** @param {SubmitEvent} event */
  function deleteAccount(event) {
    event.preventDefault();
    return doing(async () => {
      await accountsDeletionsCreate(currentUser.value?.kind === "trial" ? null : code.trim());
      currentUserForget();
      if (hasDeviceAccounts) {
        await navigateToUrl(accountsIndexUrl());
        flashMessageShow(t`This account was deleted, and removed from this device.`);
      } else {
        window.location.assign(PUBLIC_HOME_PATH);
      }
    });
  }
</script>

<Page>
  <Heading level={1}>{t`Delete my account`}</Heading>
  {#if currentUser.value?.kind === "local"}
    <Text>{t`This account is only on this device, with nothing on the server to delete.`}</Text>
    {#snippet accountsLink()}<Link href={accountsIndexUrl()}>{t`this device's accounts`}</Link>{/snippet}
    <Text><TranslatedParts parts={jt`To remove it and its data, remove it from ${accountsLink}.`} /></Text>
  {:else if currentUser.value}
    <Text>
      {t`Deleting your account deletes everything you made, in every community and on every device, and your purchases. It cannot be undone.`}
    </Text>
    {#if currentUser.value.plan?.store === "stripe"}
      <Text tone="warning">{t`Your plan, ${currentUser.value.plan.product_name}, ends now, with no refund.`}</Text>
    {:else if currentUser.value.plan?.store === "app_store" || currentUser.value.plan?.store === "play_store"}
      {@const store = currentUser.value.plan.store === "app_store" ? "Apple" : "Google Play"}
      <Text tone="warning">
        {t`Your plan, ${currentUser.value.plan.product_name}, is billed by ${store} until you cancel it there. Deleting your account does not cancel it.`}
        <Link href={STORE_SUBSCRIPTIONS_URLS[currentUser.value.plan.store]} target="_blank" rel="noopener">
          {currentUser.value.plan.store === "app_store" ? t`Manage it in the App Store` : t`Manage it in Google Play`}
        </Link>
      </Text>
    {/if}
    <form onsubmit={deleteAccount} class="flex flex-col gap-3">
      {#if currentUser.value.kind === "full"}
        <Text>{t`We email you a code to make sure it is you.`}</Text>
        <div>
          <Button onclick={sendCode} disabled={busy}>{codeSent ? t`Send a new code` : t`Send code`}</Button>
        </div>
        {#if codeSent}
          <Text tone="muted">{t`A code is on its way to ${currentUser.value.email}. It expires in ten minutes.`}</Text>
          <CodeInput bind:value={code} disabled={busy} />
        {/if}
      {/if}
      <ErrorMessage message={error} />
      <div class="flex flex-wrap items-center gap-3">
        {#if currentUser.value.kind === "trial" || codeSent}
          <Button variant="danger" type="submit" disabled={busy || (currentUser.value.kind === "full" && code.length < 6)}>
            {t`Delete my account`}
          </Button>
        {/if}
        <Link size="sm" href={accountsCurrentShowUrl()}>{t`Back to account`}</Link>
      </div>
    </form>
  {:else if error}
    <ErrorMessage message={error} />
  {/if}
</Page>
