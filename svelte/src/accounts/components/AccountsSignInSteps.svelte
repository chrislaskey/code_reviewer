<!-- What follows the email on the Sign in page: the code that was
     emailed, checked as soon as its six digits are in, then one button,
     "Create account" for a new email or "Sign in" for an account, with
     the account's password above it when it has one, or a new one in
     place of a forgotten one when the app lets it be reset. Done, the
     person sees the posts. With a device's challenge it ends in a code
     for the device instead, see docs/design/accounts-sign-in.md. -->
<script>
  import { tick } from "svelte";
  import PublicPagesLinks from "../../lib/PublicPagesLinks.svelte";
  import { accountsCode, accountsCodeResend, accountsSignInConfirm, accountsSignInResetPassword } from "../../lib/core/index.js";
  import { errorMessageOf } from "../../lib/error_message.js";
  import { currentUserForget } from "../../lib/current_user.svelte.js";
  import { navigateToUrl } from "../../lib/router.js";
  import { postsIndexUrl } from "../../lib/routes.js";
  import AccountsDeviceCode from "./AccountsDeviceCode.svelte";
  import { Button, CodeInput, ErrorMessage, Input, Link, Text } from "../../lib/components/index.js";

  /**
   * @typedef {Object} Props
   * @property {string} email where the code went
   * @property {string} pending the pending sign-in the email step answered
   * @property {string} [challenge] the device's, when it opened the page
   * @property {{appPort?: string, appScheme?: string}} [appReturn] where to return the device code to, see accountsAppReturnUrl
   * @property {boolean} [keepSignedIn] whether the browser stays signed in after it closes
   * @property {() => void} onstartover back to the email
   */

  /** @type {Props} */
  let { email, pending, challenge = "", appReturn = {}, keepSignedIn = false, onstartover } = $props();

  /** What the code answered once it matched. */
  let matched = $state(/** @type {import("../../lib/core/http-api.js").CodeMatched | null} */ (null));
  /** The person said they forgot their password and makes a new one. */
  let resetting = $state(false);
  let code = $state("");
  let password = $state("");
  let deviceCode = $state("");
  let resent = $state(false);
  let error = $state("");
  let busy = $state(false);
  /** @type {CodeInput | undefined} */
  let codeInput = $state();

  let submitText = $derived(resetting ? "Reset password and sign in" : matched?.new_account ? "Create account" : "Sign in");

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

  /** @param {import("../../lib/core/http-api.js").SignInFinished} finished */
  async function finish(finished) {
    if ("code" in finished) {
      deviceCode = finished.code;
      return;
    }
    currentUserForget();
    await navigateToUrl(postsIndexUrl());
  }

  /** @param {string} complete */
  async function checkCode(complete) {
    await attempt(async () => {
      try {
        matched = await accountsCode(pending, complete);
      } catch (problem) {
        code = "";
        throw problem;
      }
    });
    await focusCode();
  }

  async function resend() {
    await attempt(async () => {
      resent = false;
      await accountsCodeResend(pending);
      resent = true;
      code = "";
    });
    await focusCode();
  }

  /** Back to the boxes once they are enabled again, while the code is still to enter. */
  async function focusCode() {
    await tick();
    if (!matched) codeInput?.focus();
  }

  /** @param {SubmitEvent} event */
  function submit(event) {
    event.preventDefault();
    if (!matched) return;
    const next = matched.pending;
    return attempt(async () => {
      if (resetting) await finish(await accountsSignInResetPassword(next, password, challenge, keepSignedIn));
      else await finish(await accountsSignInConfirm(next, matched?.needs_password ? password : null, challenge, keepSignedIn));
    });
  }

  function forgot() {
    resetting = true;
    password = "";
    error = "";
  }
</script>

{#if deviceCode}
  <AccountsDeviceCode code={deviceCode} {appReturn} />
{:else}
  <form onsubmit={submit} class="flex flex-col gap-3">
    <Text>We sent a code to <strong>{email}</strong>. Enter it to continue; it expires in ten minutes.</Text>
    <CodeInput bind:this={codeInput} bind:value={code} disabled={busy || !!matched} oncomplete={checkCode} />
    {#if matched}
      <Text tone="muted" size="sm">Code confirmed.</Text>
    {:else if busy && code.length === 6}
      <Text tone="muted" size="sm">Checking the code…</Text>
    {/if}
    {#if resent}
      <Text tone="muted" size="sm">A new code is on its way; the old one no longer works.</Text>
    {/if}
    {#if matched?.needs_password}
      {#if resetting}
        <Text>Choose a new password to finish signing in; the old one stops working.</Text>
      {/if}
      <Input
        bind:value={password}
        type="password"
        autocomplete={resetting ? "new-password" : "current-password"}
        minlength={resetting ? 12 : undefined}
        aria-label={resetting ? "New password" : "Password"}
        placeholder={resetting ? "New password, 12 characters or more" : "Password"}
        required
      />
    {/if}
    <ErrorMessage message={error} />
    <Text tone="muted" size="sm">By continuing, you accept the <PublicPagesLinks />.</Text>
    <div class="flex flex-wrap items-center gap-3">
      <Button variant="primary" type="submit" disabled={busy || !matched}>{matched ? submitText : "Continue"}</Button>
      {#if matched?.can_reset_password && !resetting}
        <Link size="sm" onclick={forgot} disabled={busy}>Forgot your password?</Link>
      {/if}
      {#if !matched}
        <Link size="sm" onclick={resend} disabled={busy}>Send a new code</Link>
      {/if}
      <Link size="sm" onclick={onstartover} disabled={busy}>Use another email</Link>
    </div>
  </form>
{/if}
