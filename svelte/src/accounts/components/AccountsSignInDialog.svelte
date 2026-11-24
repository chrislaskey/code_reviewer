<!-- Shown while a sign-in waits on the browser, inside the app: the
     browser has the web's sign-in page and ends by showing a code, which
     the person pastes here, see docs/design/accounts-sign-in.md. -->
<script>
  import { jt, t } from "ttag";
  import { Button, Dialog, DialogActions, ErrorMessage, Heading, Input, Text, TranslatedParts } from "../../lib/components/index.js";
  /**
   * @typedef {Object} Props
   * @property {{email: string | null} | null} pending the sign-in waiting; the dialog is open while set
   * @property {string} [error]
   * @property {boolean} [busy]
   * @property {(code: string) => void} onfinish
   * @property {() => void} onopenbrowser the browser again, with a fresh start
   * @property {() => void} oncancel
   */

  /** @type {Props} */
  let { pending, error = "", busy = false, onfinish, onopenbrowser, oncancel } = $props();

  /** @type {HTMLDialogElement | undefined} */
  let dialog = $state();
  let code = $state("");

  $effect(() => {
    if (!dialog) return;
    if (pending && !dialog.open) {
      code = "";
      dialog.showModal();
    }
    if (!pending && dialog.open) dialog.close();
  });

  function submit(event) {
    event.preventDefault();
    onfinish(code.trim());
  }
</script>

<Dialog bind:element={dialog} onclose={() => pending && oncancel()}>
  {#if pending}
    <Heading>{t`Finish logging in in your browser`}</Heading>
    <Text>
      {#if pending.email}
        {#snippet strongEmail()}<strong>{pending.email}</strong>{/snippet}
        <TranslatedParts parts={jt`The page to log in as ${strongEmail} opened in your browser.`} />
      {:else}
        {t`The page to log in opened in your browser.`}
      {/if}
      {t`When it shows a code, paste it here.`}
    </Text>
    <form onsubmit={submit} class="flex flex-col gap-3">
      <Input bind:value={code} autocomplete="off" aria-label={t`Code from the browser`} placeholder={t`Code from the browser`} required />
      <ErrorMessage message={error} />
      <DialogActions>
        <Button size="sm" variant="ghost" onclick={oncancel} disabled={busy}>{t`Cancel`}</Button>
        <Button size="sm" onclick={onopenbrowser} disabled={busy}>{t`Open the browser again`}</Button>
        <Button size="sm" variant="primary" type="submit" disabled={busy}>{t`Finish`}</Button>
      </DialogActions>
    </form>
  {/if}
</Dialog>
