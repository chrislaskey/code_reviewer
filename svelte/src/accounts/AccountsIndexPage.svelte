<!-- The accounts on this device, at /accounts, inside the app: the active
     one, the others to switch to or sign in to, each to remove from the
     device, and Add account. The device section of the app: no account
     is open while the person adds one, which happens in the browser,
     with the dialog waiting for its code, pasted or returned by itself
     by a desktop's browser; on an iPhone the sign-in sheet over the app
     returns it, with no dialog behind it. On a device with no account,
     Skip makes a local user, on this device only; when they later sign
     up or in, they are asked whether what they made moves into the
     account, see docs/design/accounts-local-and-trial-users.md. -->
<script>
  import {
    accountsSignInCancel,
    accountsSignInFinish,
    accountsSignInPending,
    accountsSignInStart,
    deviceAccountsDeactivate,
    deviceAccountsList,
    deviceAccountsRemove,
    deviceAccountsSwitch,
    localUsersCanCreate,
    localUsersCreate,
    localUsersKeepSeparate,
    localUsersMove,
    onAccountsSignInReturned,
  } from "../lib/core/index.js";
  import { errorMessageOf } from "../lib/error_message.js";
  import { currentUserForget } from "../lib/current_user.svelte.js";
  import { navigateToUrl } from "../lib/router.js";
  import { postsIndexUrl } from "../lib/routes.js";
  import AccountsMoveDialog from "./components/AccountsMoveDialog.svelte";
  import AccountsRemoveDialog from "./components/AccountsRemoveDialog.svelte";
  import AccountsSignInDialog from "./components/AccountsSignInDialog.svelte";
  import { Badge, Button, ErrorMessage, Heading, Page, Panel, Text } from "../lib/components/index.js";

  /** @typedef {{id: string, server_id: string | null, user_id: string, email: string, signed_in: boolean, local: boolean, active: boolean, last_active_at: string}} DeviceAccount */

  /** @type {DeviceAccount[]} */
  let accounts = $state([]);
  /** @type {DeviceAccount | null} */
  let removing = $state(null);
  /** @type {{pending: number, conflicts: number} | null} */
  let loss = $state(null);
  /** @type {{email: string | null, in_sheet?: boolean} | null} */
  let signingIn = $state(null);
  let signInError = $state("");
  let canSkip = $state(false);
  /** @type {{email: string, items: number} | null} */
  let moveOffered = $state(null);
  let moveError = $state("");
  let busy = $state(false);
  let error = $state("");
  /** Whether the page was opened just after an account was deleted. */
  const accountDeleted = new URLSearchParams(window.location.search).has("account_deleted");

  async function load() {
    try {
      accounts = await deviceAccountsList();
      canSkip = await localUsersCanCreate();
      signingIn = await accountsSignInPending();
    } catch (problem) {
      error = errorMessageOf(problem);
    }
  }

  /** @param {() => Promise<unknown>} action */
  async function doThen(action) {
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

  /** @param {DeviceAccount} account */
  function switchTo(account) {
    return doThen(async () => {
      await deviceAccountsSwitch(account.id);
      currentUserForget();
      await navigateToUrl(postsIndexUrl());
    });
  }

  /**
   * Closes the active account and opens the browser to sign in, then
   * waits for the code in the dialog.
   * @param {string | null} email
   */
  function openBrowser(email) {
    return doThen(async () => {
      await deviceAccountsDeactivate();
      currentUserForget();
      signInError = "";
      const started = await accountsSignInStart(email ?? undefined);
      signingIn = { email, in_sheet: started.in_sheet };
      accounts = await deviceAccountsList();
    });
  }

  /** @param {string} code */
  function finishSignIn(code) {
    return doThen(async () => {
      signInError = "";
      let finished;
      try {
        finished = await accountsSignInFinish(code);
      } catch (problem) {
        signInError = errorMessageOf(problem);
        return;
      }
      await signedIn(finished);
    });
  }

  /** @param {{outcome: "signed_in"} | {outcome: "move_offered", email: string, items: number}} finished */
  async function signedIn(finished) {
    signingIn = null;
    if (finished.outcome === "move_offered") {
      moveError = "";
      moveOffered = { email: finished.email, items: finished.items };
      return;
    }
    currentUserForget();
    await navigateToUrl(postsIndexUrl());
  }

  $effect(() =>
    onAccountsSignInReturned((returned) =>
      doThen(async () => {
        if (returned.result === "cancelled" || (returned.result === "failed" && signingIn?.in_sheet)) {
          signingIn = null;
          if (returned.result === "failed") error = returned.error;
          accounts = await deviceAccountsList();
          return;
        }
        if (returned.result === "failed") {
          signInError = returned.error;
          return;
        }
        await signedIn(returned.finished);
      }),
    ),
  );

  /** Skip: a local user, made and opened with no request. */
  function skip() {
    return doThen(async () => {
      await localUsersCreate();
      currentUserForget();
      await navigateToUrl(postsIndexUrl());
    });
  }

  /** @param {() => Promise<unknown>} answer localUsersMove or localUsersKeepSeparate */
  function answerMove(answer) {
    return doThen(async () => {
      moveError = "";
      try {
        await answer();
      } catch (problem) {
        moveError = errorMessageOf(problem);
        return;
      }
      moveOffered = null;
      currentUserForget();
      await navigateToUrl(postsIndexUrl());
    });
  }

  function cancelSignIn() {
    return doThen(async () => {
      signingIn = null;
      signInError = "";
      await accountsSignInCancel();
    });
  }

  /** @param {boolean} losingChanges */
  function remove(losingChanges) {
    const account = removing;
    if (!account) return;
    return doThen(async () => {
      const answer = await deviceAccountsRemove(account.id, losingChanges);
      if (answer.outcome === "changes_would_be_lost") {
        loss = { pending: answer.pending, conflicts: answer.conflicts };
        return;
      }
      removing = null;
      loss = null;
      if (account.active) currentUserForget();
      await load();
    });
  }

  load();
</script>

<Page>
  <Heading level={1}>Accounts on this device</Heading>
  {#if accountDeleted}
    <Text tone="muted">This account was deleted, and removed from this device.</Text>
  {/if}

  <ErrorMessage message={error} />

  {#if accounts.length === 0 && !error}
    <Text tone="muted">No account yet.</Text>
  {/if}

  <ul class="flex flex-col gap-3">
    {#each accounts as account (account.id)}
      <Panel as="li" layout="row" class="flex-wrap">
        <span class="grow">{account.local ? "On this device only" : account.email}</span>
        {#if account.local}
          <Button size="sm" variant="primary" onclick={() => openBrowser(null)} disabled={busy}>
            Sign up or sign in
          </Button>
        {/if}
        {#if account.active}
          <Badge tone="primary">Active</Badge>
        {:else if account.signed_in}
          <Button size="sm" onclick={() => switchTo(account)} disabled={busy}>Switch</Button>
        {:else}
          <Badge tone="ghost">Signed out</Badge>
          <Button size="sm" onclick={() => openBrowser(account.email)} disabled={busy}>Sign in</Button>
        {/if}
        <Button size="sm" variant="ghost" onclick={() => ((loss = null), (removing = account))} disabled={busy}>
          Remove from device
        </Button>
      </Panel>
    {/each}
  </ul>

  <div class="flex flex-wrap items-center gap-3">
    <Button size="sm" onclick={() => openBrowser(null)} disabled={busy}>
      {accounts.length === 0 ? "Sign up or sign in" : "Add account"}
    </Button>
    {#if canSkip}
      <Button size="sm" variant="ghost" onclick={skip} disabled={busy} data-skip>Skip</Button>
      <Text as="span" tone="muted" size="sm">Use the app on this device only, with no account. You can sign up later.</Text>
    {/if}
  </div>

  <AccountsSignInDialog
    pending={signingIn?.in_sheet ? null : signingIn}
    error={signInError}
    {busy}
    onfinish={finishSignIn}
    onopenbrowser={() => openBrowser(signingIn?.email ?? null)}
    oncancel={cancelSignIn}
  />

  <AccountsMoveDialog
    offered={moveOffered}
    error={moveError}
    {busy}
    onmove={() => answerMove(localUsersMove)}
    onkeepseparate={() => answerMove(localUsersKeepSeparate)}
  />

  <AccountsRemoveDialog
    account={removing}
    {loss}
    {busy}
    onremove={remove}
    oncancel={() => ((removing = null), (loss = null))}
  />
</Page>
