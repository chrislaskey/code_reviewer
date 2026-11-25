<!-- The accounts on this device, at /accounts, inside the app: the active
     one, the others to switch to or sign in to, each to remove from the
     device, and Add account. A tap anywhere on an account but its other
     buttons opens it: the active one and a signed-in one on the page last
     visited in it, a logged-out one at Log in. The device section of the app: no account
     is open while the person adds one, which happens in the browser,
     with the dialog waiting for its code, pasted or returned by itself
     by a desktop's browser; on an iPhone the sign-in sheet over the app
     returns it, with no dialog behind it. On a device with no account,
     Skip makes a local user, on this device only; when they later sign
     up or in, they are asked whether what they made moves into the
     account, see docs/design/accounts-local-and-trial-users.md. Sign up
     is offered only where the server takes new sign-ups. -->
<script>
  import { t } from "ttag";
  import {
    accountsSignInCancel,
    accountsSignInFinish,
    accountsSignInPending,
    accountsSignInStart,
    deviceAccountsDeactivate,
    deviceAccountsList,
    deviceAccountsRemove,
    deviceAccountsSwitch,
    lastVisitedPageGet,
    localUsersCanCreate,
    localUsersCreate,
    localUsersKeepSeparate,
    localUsersMove,
    multiServerGet,
    onAccountsSignInReturned,
  } from "../lib/core/index.js";
  import { errorMessageOf } from "../lib/error_message.js";
  import { currentUserForget } from "../lib/current_user.svelte.js";
  import { currentServer, currentServerLoad } from "../lib/current_server.svelte.js";
  import { navigateToUrl } from "../lib/router.js";
  import { pageAfterSignInUrl } from "../lib/pages.js";
  import AccountsCustomServer from "./components/AccountsCustomServer.svelte";
  import AccountsMoveDialog from "./components/AccountsMoveDialog.svelte";
  import AccountsRemoveDialog from "./components/AccountsRemoveDialog.svelte";
  import AccountsSignInDialog from "./components/AccountsSignInDialog.svelte";
  import { Badge, Button, ErrorMessage, Heading, Icon, Page, Panel, Text } from "../lib/components/index.js";

  /** @typedef {{id: string, server_id: string | null, server_host: string | null, server_display_name: string | null, user_id: string, email: string, signed_in: boolean, local: boolean, active: boolean, last_active_at: string}} DeviceAccount */

  /** @type {DeviceAccount[]} */
  let accounts = $state([]);
  /** @type {DeviceAccount | null} */
  let removing = $state(null);
  /** @type {{pending: number, conflicts: number} | null} */
  let loss = $state(null);
  /** @type {{email: string | null, server_id: string | null, in_sheet?: boolean} | null} */
  let signingIn = $state(null);
  /** @type {"none" | "listed_only" | "any"} */
  let multiServer = $state("none");
  let signInError = $state("");
  let canSkip = $state(false);
  /** @type {{email: string, items: number} | null} */
  let pendingMove = $state(null);
  let moveError = $state("");
  let busy = $state(false);
  let error = $state("");
  currentServerLoad().catch(() => {});
  /** Whether the server takes new sign-ups, as its settings say. */
  let signUpEnabled = $derived(currentServer.value?.settings.features.accounts_sign_up ?? true);
  let signUpOrSignIn = $derived(signUpEnabled ? t`Sign up or log in` : t`Log in`);

  async function load() {
    try {
      accounts = await deviceAccountsList();
      canSkip = await localUsersCanCreate();
      multiServer = await multiServerGet();
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
      const switched = await deviceAccountsSwitch(account.id);
      currentUserForget();
      await navigateToUrl(switched.last_visited_page ?? pageAfterSignInUrl());
    });
  }

  /** The active account, on the page last visited in it. */
  function openActive() {
    return doThen(async () => {
      await navigateToUrl((await lastVisitedPageGet()) ?? pageAfterSignInUrl());
    });
  }

  /** @param {DeviceAccount} account */
  function open(account) {
    if (account.active) return openActive();
    if (account.signed_in) return switchTo(account);
    return openBrowser(account.email, account.server_id);
  }

  /**
   * Closes the active account and opens the browser to sign in, then
   * waits for the code in the dialog: on the server, the app's own when
   * none.
   * @param {string | null} email
   * @param {string | null} [serverId]
   */
  function openBrowser(email, serverId = null) {
    return doThen(async () => {
      await deviceAccountsDeactivate();
      currentUserForget();
      signInError = "";
      const started = await accountsSignInStart(email ?? undefined, serverId ?? undefined);
      signingIn = { email, server_id: serverId, in_sheet: started.in_sheet };
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

  /** @param {{outcome: "signed_in"} | {outcome: "pending_move", email: string, items: number}} finished */
  async function signedIn(finished) {
    signingIn = null;
    if (finished.outcome === "pending_move") {
      moveError = "";
      pendingMove = { email: finished.email, items: finished.items };
      return;
    }
    currentUserForget();
    await navigateToUrl(pageAfterSignInUrl());
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
      await navigateToUrl(pageAfterSignInUrl());
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
      pendingMove = null;
      currentUserForget();
      await navigateToUrl(pageAfterSignInUrl());
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
  <Heading level={1}>{t`Accounts on this device`}</Heading>

  <ErrorMessage message={error} />

  {#if accounts.length === 0 && !error}
    <Text tone="muted">{t`No account yet.`}</Text>
  {/if}

  <ul class="flex flex-col gap-3">
    {#each accounts as account (account.id)}
      <!-- The account's name is a button whose ::after covers the panel,
           so the whole panel opens it; its other buttons sit above. -->
      <Panel as="li" layout="row" class="relative flex-wrap hover:bg-base-200">
        <button
          type="button"
          class="flex grow cursor-pointer flex-col text-start after:absolute after:inset-0 after:rounded-[inherit] after:content-['']"
          onclick={() => open(account)}
          disabled={busy}
          data-account-open
        >
          <span>{account.local ? t`On this device only` : account.email}</span>
          {#if account.server_host}
            <Text as="span" tone="muted" size="sm">{account.server_display_name} · {account.server_host}</Text>
          {/if}
        </button>
        {#if account.local}
          <Button size="sm" variant="primary" class="relative" onclick={() => openBrowser(null)} disabled={busy}>
            {signUpOrSignIn}
          </Button>
        {/if}
        {#if account.active}
          <Badge tone="primary">{t`Active`}</Badge>
        {:else if account.signed_in}
          <Button size="sm" class="relative" onclick={() => switchTo(account)} disabled={busy}>{t`Switch`}</Button>
        {:else}
          <Badge tone="ghost">{t`Logged out`}</Badge>
          <Button size="sm" class="relative" onclick={() => openBrowser(account.email, account.server_id)} disabled={busy}>{t`Log in`}</Button>
        {/if}
        <Button size="sm" variant="ghost" class="relative" onclick={() => ((loss = null), (removing = account))} disabled={busy}>
          {t`Remove from device`}
        </Button>
        <Icon name="hero-chevron-right" class="text-base-content/50 size-4" />
      </Panel>
    {/each}
  </ul>

  <div class="flex flex-wrap items-center gap-3">
    <Button size="sm" onclick={() => openBrowser(null)} disabled={busy}>
      {accounts.length === 0 ? signUpOrSignIn : t`Add account`}
    </Button>
    {#if canSkip}
      <Button size="sm" variant="ghost" onclick={skip} disabled={busy} data-skip>{t`Skip`}</Button>
      <Text as="span" tone="muted" size="sm">{t`Use the app on this device only, with no account. You can sign up later.`}</Text>
    {/if}
  </div>

  {#if multiServer !== "none"}
    <AccountsCustomServer {busy} onsignin={(serverId) => openBrowser(null, serverId)} />
  {/if}

  <AccountsSignInDialog
    pending={signingIn?.in_sheet ? null : signingIn}
    error={signInError}
    {busy}
    onfinish={finishSignIn}
    onopenbrowser={() => openBrowser(signingIn?.email ?? null, signingIn?.server_id ?? null)}
    oncancel={cancelSignIn}
  />

  <AccountsMoveDialog
    {pendingMove}
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
