<!-- What follows "Email a code" on the Log in page: the code that was
     emailed, checked as soon as its six digits are in, then one button,
     "Create account" for a new email or "Log in" for an account, and the
     choice to stay logged in under it. The page finishes the sign-in, see
     docs/design/accounts-sign-in.md. -->
<script>
  import { jt, t } from "ttag";
  import { tick } from "svelte";
  import { accountsCode, accountsCodeResend, accountsSignInConfirm } from "../../lib/core/index.js";
  import { errorMessageOf } from "../../lib/error_message.js";
  import { Button, Checkbox, CodeInput, ErrorMessage, Link, Text, TranslatedParts } from "../../lib/components/index.js";

  /**
   * @typedef {Object} Props
   * @property {string} email where the code went
   * @property {string} pending the pending sign-in the email step answered
   * @property {string} [challenge] the device's, when it opened the page
   * @property {boolean} [keepSignedIn] whether the browser stays logged in after it closes
   * @property {(finished: import("../../lib/core/http-api.js").SignInFinished) => Promise<void>} onfinish
   * @property {() => void} onstartover back to the email
   */

  /** @type {Props} */
  let { email, pending, challenge = "", keepSignedIn = $bindable(false), onfinish, onstartover } = $props();

  /** What the code answered once it matched. */
  let matched = $state(/** @type {import("../../lib/core/http-api.js").CodeMatched | null} */ (null));
  let code = $state("");
  let resent = $state(false);
  let error = $state("");
  let busy = $state(false);
  /** @type {CodeInput | undefined} */
  let codeInput = $state();

  let submitText = $derived(matched?.new_account ? t`Create account` : t`Log in`);

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
    return attempt(async () => onfinish(await accountsSignInConfirm(next, challenge, keepSignedIn)));
  }
</script>

<form onsubmit={submit} class="flex flex-col gap-3">
  {#snippet strongEmail()}<strong>{email}</strong>{/snippet}
  <Text>
    <TranslatedParts parts={jt`We sent a code to ${strongEmail}. It expires in ten minutes.`} />
    <Link onclick={onstartover} disabled={busy}>{t`Change email`}</Link>
  </Text>
  <CodeInput bind:this={codeInput} bind:value={code} disabled={busy || !!matched} oncomplete={checkCode} />
  {#if matched}
    <Text tone="muted" size="sm">{t`Code confirmed.`}</Text>
  {:else if busy && code.length === 6}
    <Text tone="muted" size="sm">{t`Checking the code…`}</Text>
  {/if}
  {#if resent}
    <Text tone="muted" size="sm">{t`A new code is on its way; the old one no longer works.`}</Text>
  {/if}
  <ErrorMessage message={error} />
  <Button variant="primary" type="submit" disabled={busy || !matched}>{matched ? submitText : t`Continue`}</Button>
  {#if !challenge}
    <Checkbox bind:checked={keepSignedIn} size="sm" tone="muted">{t`Keep me logged in`}</Checkbox>
  {/if}
  {#if !matched}
    <div><Link size="sm" onclick={resend} disabled={busy}>{t`Send a new code`}</Link></div>
  {/if}
</form>
