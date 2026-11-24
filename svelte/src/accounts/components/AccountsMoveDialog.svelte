<!-- Asked after a sign-in on a device that has a local user: whether
     what they made moves into the account that just signed in, or the
     two stay apart, see docs/design/accounts-local-and-trial-users.md. -->
<script>
  import { jt, msgid, ngettext, t } from "ttag";
  import { Button, Dialog, DialogActions, ErrorMessage, Heading, Text, TranslatedParts } from "../../lib/components/index.js";
  /**
   * @typedef {Object} Props
   * @property {{email: string, items: number} | null} offered the move offered; the dialog is open while set
   * @property {string} [error]
   * @property {boolean} [busy]
   * @property {() => void} onmove
   * @property {() => void} onkeepseparate
   */

  /** @type {Props} */
  let { offered, error = "", busy = false, onmove, onkeepseparate } = $props();

  /** @type {HTMLDialogElement | undefined} */
  let dialog = $state();

  $effect(() => {
    if (!dialog) return;
    if (offered && !dialog.open) dialog.showModal();
    if (!offered && dialog.open) dialog.close();
  });
</script>

<Dialog bind:element={dialog} oncancel={(event) => event.preventDefault()}>
  {#if offered}
    <Heading>
      {ngettext(
        msgid`Move ${offered.items} item into ${offered.email}?`,
        `Move ${offered.items} items into ${offered.email}?`,
        offered.items,
      )}
    </Heading>
    {#snippet strongEmail()}<strong>{offered.email}</strong>{/snippet}
    <Text>
      <TranslatedParts parts={jt`What you made on this device becomes part of ${strongEmail} and syncs with it.`} />
      {t`The account that was only on this device is gone afterwards.`}
    </Text>
    <Text tone="muted" size="sm">
      {t`Keeping them separate adds ${offered.email} beside it, and leaves what you made where it is, on this device only.`}
    </Text>
    <ErrorMessage message={error} />
    <DialogActions>
      <Button size="sm" variant="ghost" onclick={onkeepseparate} disabled={busy}>{t`Keep them separate`}</Button>
      <Button size="sm" variant="primary" onclick={onmove} disabled={busy} data-move>{t`Move`}</Button>
    </DialogActions>
  {/if}
</Dialog>
