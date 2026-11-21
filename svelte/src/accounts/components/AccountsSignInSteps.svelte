<!-- What follows the email on the Sign up and Sign in pages: the code
     that was emailed, then the account's password when it has one, or a
     new one in its place when it was forgotten and the app lets it be
     reset, or a password to make or skip at a first sign-in; done, the person sees
     the posts. With a device's challenge it ends in a code for the
     device instead, see docs/design/accounts-sign-in.md. -->
<script>
  import { accountsCode, accountsCodeResend, accountsSignInNewPassword, accountsSignInPassword, accountsSignInResetPassword } from "../../lib/core/index.js";
  import { currentUserForget } from "../../lib/current_user.js";
  import { navigateToUrl } from "../../lib/router.js";
  import { postsIndexUrl } from "../../lib/routes.js";
  import AccountsCodeForm from "./AccountsCodeForm.svelte";
  import AccountsDeviceCode from "./AccountsDeviceCode.svelte";
  import AccountsPasswordForm from "./AccountsPasswordForm.svelte";

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

  /** The step after the code, once the code answered one. */
  let next = $state(/** @type {"password" | "new_password" | null} */ (null));
  let nextPending = $state("");
  let canResetPassword = $state(false);
  /** The person said they forgot their password and makes a new one. */
  let resetting = $state(false);
  let code = $state("");
  let password = $state("");
  let deviceCode = $state("");
  let resent = $state(false);
  let error = $state("");
  let busy = $state(false);

  /** @param {() => Promise<void>} action */
  async function attempt(action) {
    busy = true;
    error = "";
    try {
      await action();
    } catch (problem) {
      error = problem instanceof Error ? problem.message : String(problem);
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

  function submitCode() {
    return attempt(async () => {
      const answer = await accountsCode(pending, code, challenge, keepSignedIn);
      if ("next" in answer) {
        next = answer.next;
        nextPending = answer.pending;
        canResetPassword = answer.can_reset_password;
        return;
      }
      await finish(answer);
    });
  }

  function resend() {
    return attempt(async () => {
      resent = false;
      await accountsCodeResend(pending);
      resent = true;
      code = "";
    });
  }

  function submitPassword() {
    return attempt(async () => {
      const send = next === "new_password" ? accountsSignInNewPassword : resetting ? accountsSignInResetPassword : accountsSignInPassword;
      await finish(await send(nextPending, password, challenge, keepSignedIn));
    });
  }

  function forgot() {
    resetting = true;
    password = "";
    error = "";
  }

  function skipPassword() {
    return attempt(async () => finish(await accountsSignInNewPassword(nextPending, null, challenge, keepSignedIn)));
  }
</script>

{#if deviceCode}
  <AccountsDeviceCode code={deviceCode} {appReturn} />
{:else if next}
  <AccountsPasswordForm
    isNew={next === "new_password"}
    {resetting}
    bind:password
    {error}
    {busy}
    onsubmit={submitPassword}
    onskip={skipPassword}
    onforgot={canResetPassword ? forgot : undefined}
    {onstartover}
  />
{:else}
  <AccountsCodeForm {email} bind:code {error} {busy} {resent} onsubmit={submitCode} onresend={resend} {onstartover} />
{/if}
