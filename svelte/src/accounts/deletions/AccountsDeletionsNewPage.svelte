<!-- Deleting the signed-in person's account, at /accounts/deletions/new:
     what goes, the plan's warning by where it was bought, Send code, the
     code and Delete my account, see docs/design/accounts-delete.md. A
     trial is deleted with no code. After it, the start page: the
     public home on the web, the device's accounts in the app. A local
     user has no account; removing the device's data is the accounts
     list's. -->
<script>
  import { accountsDeletionsCodeCreate, accountsDeletionsCreate, hasDeviceAccounts } from "../../lib/core/index.js";
  import { errorMessageOf } from "../../lib/error_message.js";
  import { currentUser, currentUserForget, currentUserLoad } from "../../lib/current_user.svelte.js";
  import { navigateToUrl } from "../../lib/router.js";
  import { PUBLIC_HOME_PATH, accountsIndexUrl, accountsCurrentShowUrl } from "../../lib/routes.js";
  import { Button, ErrorMessage, Heading, Input, Link, Page, Text } from "../../lib/components/index.js";

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
      if (hasDeviceAccounts) await navigateToUrl(accountsIndexUrl({ accountDeleted: true }));
      else window.location.assign(PUBLIC_HOME_PATH);
    });
  }
</script>

<Page>
  <Heading level={1}>Delete my account</Heading>
  {#if currentUser.value?.kind === "local"}
    <Text>This account is only on this device, with nothing on the server to delete.</Text>
    <Text>To remove it and its data, remove it from <Link href={accountsIndexUrl()}>this device's accounts</Link>.</Text>
  {:else if currentUser.value}
    <Text>
      Deleting your account deletes everything you made, in every community and on every device, and your
      purchases. It cannot be undone.
    </Text>
    {#if currentUser.value.plan?.store === "stripe"}
      <Text tone="warning">Your plan, {currentUser.value.plan.product_name}, ends now, with no refund.</Text>
    {:else if currentUser.value.plan?.store === "app_store" || currentUser.value.plan?.store === "play_store"}
      {@const store = currentUser.value.plan.store === "app_store" ? "Apple" : "Google Play"}
      <Text tone="warning">
        Your plan, {currentUser.value.plan.product_name}, is billed by {store} until you cancel it there. Deleting your
        account does not cancel it.
        <Link href={STORE_SUBSCRIPTIONS_URLS[currentUser.value.plan.store]} target="_blank" rel="noopener">
          Manage it in {currentUser.value.plan.store === "app_store" ? "the App Store" : "Google Play"}
        </Link>
      </Text>
    {/if}
    <form onsubmit={deleteAccount} class="flex flex-col gap-3">
      {#if currentUser.value.kind === "full"}
        <Text>We email you a code to make sure it is you.</Text>
        <div>
          <Button onclick={sendCode} disabled={busy}>{codeSent ? "Send a new code" : "Send code"}</Button>
        </div>
        {#if codeSent}
          <Text tone="muted">A code is on its way to {currentUser.value.email}. It expires in ten minutes.</Text>
          <Input
            bind:value={code}
            inputmode="numeric"
            autocomplete="one-time-code"
            aria-label="Code"
            placeholder="The code from the email"
            required
          />
        {/if}
      {/if}
      <ErrorMessage message={error} />
      <div class="flex flex-wrap items-center gap-3">
        {#if currentUser.value.kind === "trial" || codeSent}
          <Button variant="danger" type="submit" disabled={busy}>Delete my account</Button>
        {/if}
        <Link size="sm" href={accountsCurrentShowUrl()}>Back to account</Link>
      </div>
    </form>
  {:else if error}
    <ErrorMessage message={error} />
  {/if}
</Page>
