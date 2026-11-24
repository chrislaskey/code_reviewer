<!-- Logging in or signing up, at /accounts/sign-in, on the web, one page
     for both: "Continue as guest" where the server has room for a trial;
     the email, then "Email a code", which goes on to the code and the
     button that makes the account or logs in to it, see
     AccountsSignInSteps, or "Use my password", which shows the password
     on this page; and the terms last, for every way in. Opened by a
     device with ?challenge=, it ends in a code for the device instead,
     see docs/design/accounts-sign-in.md. The email is filled in from
     ?email=. A server that signs in by password alone shows only the
     password. -->
<script>
  import { jt, t } from "ttag";
  import PublicPagesLinks from "../lib/PublicPagesLinks.svelte";
  import { accountsSignIn, accountsSignInPassword, accountsTrialGet } from "../lib/core/index.js";
  import { errorMessageOf } from "../lib/error_message.js";
  import { currentServer, currentServerLoad } from "../lib/current_server.svelte.js";
  import { currentUser, currentUserForget } from "../lib/current_user.svelte.js";
  import { navigateToUrl } from "../lib/router.js";
  import { pageAfterSignInUrl } from "../lib/pages.js";
  import AccountsDeviceCode from "./components/AccountsDeviceCode.svelte";
  import AccountsGuestRow from "./components/AccountsGuestRow.svelte";
  import AccountsSignInSteps from "./components/AccountsSignInSteps.svelte";
  import { KEEP_SIGNED_IN_CHECKED } from "./keep_signed_in.js";
  import { Button, Checkbox, ErrorMessage, Heading, Input, Label, Page, Text, TranslatedParts } from "../lib/components/index.js";

  const query = new URLSearchParams(window.location.search);
  const challenge = query.get("challenge") ?? "";
  const appReturn = { appPort: query.get("app_port") ?? "", appScheme: query.get("app_scheme") ?? "" };

  let email = $state(query.get("email") ?? "");
  let password = $state("");
  /** Whether the person chose "Use my password". */
  let choseWithPassword = $state(false);
  let keepSignedIn = $state(KEEP_SIGNED_IN_CHECKED);
  /** Whether to offer a trial: on the web, when the server has room for one. */
  let trialAvailable = $state(false);
  if (!challenge) accountsTrialGet().then((available) => (trialAvailable = available), () => {});
  currentServerLoad().catch(() => {});
  /** Whether the server takes new sign-ups, as its settings say. */
  let signUpEnabled = $derived(currentServer.value?.settings.features.accounts_sign_up ?? true);
  /** Whether the server signs in with an emailed code, else by password alone. */
  let signInCodeEnabled = $derived(currentServer.value?.settings.features.accounts_sign_in_code ?? true);
  let withPassword = $derived(choseWithPassword || !signInCodeEnabled);
  /** Whether the guest row shows: not to a guest, who comes here to sign up. */
  let guestShown = $derived(trialAvailable && currentUser.value?.kind !== "trial");
  let pending = $state("");
  let deviceCode = $state("");
  let error = $state("");
  let busy = $state(false);

  /** @param {() => Promise<void>} action */
  async function attempt(action) {
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

  function emailCode() {
    return attempt(async () => {
      pending = (await accountsSignIn(email)).pending;
      choseWithPassword = false;
      password = "";
    });
  }

  function logIn() {
    return attempt(async () => finish(await accountsSignInPassword(email, password, challenge, keepSignedIn)));
  }

  /** @param {import("../lib/core/http-api.js").SignInFinished} finished */
  async function finish(finished) {
    if ("code" in finished) {
      deviceCode = finished.code;
      return;
    }
    currentUserForget();
    await navigateToUrl(pageAfterSignInUrl());
  }

  /** @param {SubmitEvent} event */
  function submit(event) {
    event.preventDefault();
    return withPassword ? logIn() : emailCode();
  }

  function usePassword() {
    choseWithPassword = true;
    error = "";
  }
</script>

<Page>
  {#if deviceCode}
    <AccountsDeviceCode code={deviceCode} {appReturn} />
  {:else}
    <Heading level={1}>{pending ? t`Check your email` : signUpEnabled ? t`Log in or sign up` : t`Log in`}</Heading>
    {#if challenge}
      <Text tone="muted">{t`Logging in for the app on your device.`}</Text>
    {/if}
    {#if pending}
      <AccountsSignInSteps {email} {pending} {challenge} bind:keepSignedIn onfinish={finish} onstartover={() => (pending = "")} />
    {:else}
      {#if guestShown && !withPassword}
        <AccountsGuestRow />
        <div class="divider muted my-0 text-sm">{t`or`}</div>
      {/if}
      <form onsubmit={submit} class="flex flex-col gap-3">
        {#if withPassword}
          <Label text={t`Email`}>
            <Input bind:value={email} type="email" autocomplete="username" required />
          </Label>
          <Label text={t`Password`}>
            <Input bind:value={password} type="password" autocomplete="current-password" required />
          </Label>
          <ErrorMessage message={error} />
          <Button variant="primary" type="submit" disabled={busy}>{t`Log in`}</Button>
          {#if signInCodeEnabled}
            <Button onclick={emailCode} disabled={busy}>{t`Email a code instead`}</Button>
          {/if}
          {#if !challenge}
            <Checkbox bind:checked={keepSignedIn} size="sm" tone="muted">{t`Keep me logged in`}</Checkbox>
          {/if}
          {#if signInCodeEnabled}
            <Text tone="muted" size="sm">{t`Forgot your password? Use a code, then change it in Account settings.`}</Text>
          {:else}
            <Text tone="muted" size="sm">{t`Forgot your password? Ask whoever runs this server.`}</Text>
          {/if}
        {:else}
          <Input bind:value={email} type="email" autocomplete="username" aria-label={t`Email`} placeholder={t`Email`} required />
          <ErrorMessage message={error} />
          <Button variant="primary" type="submit" disabled={busy}>{t`Email a code`}</Button>
          <Button onclick={usePassword} disabled={busy}>{t`Use my password`}</Button>
          {#if !signUpEnabled}
            <Text tone="muted" size="sm">{t`This server takes no new sign-ups.`}</Text>
          {/if}
        {/if}
      </form>
    {/if}
    <div class="border-base-300 border-t pt-3 text-center">
      {#snippet publicPagesLinks()}<PublicPagesLinks />{/snippet}
      <Text tone="muted" size="sm"><TranslatedParts parts={jt`By continuing, you accept the ${publicPagesLinks}.`} /></Text>
    </div>
  {/if}
</Page>
