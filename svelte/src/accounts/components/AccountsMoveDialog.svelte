<!-- Asked after a sign-in on a device that has a local user: whether
     what they made moves into the account that just signed in, or the
     two stay apart, see docs/design/accounts-local-and-trial-users.md. -->
<script>
  import { jt, msgid, ngettext, t } from "ttag";
  import { Button, Dialog, DialogActions, ErrorMessage, Heading, Text, TranslatedParts } from "../../lib/components/index.js";
  /**
   * @typedef {Object} Props
   * @property {{email: string, items: number} | null} pendingMove the move the person is asked about; the dialog is open while set
   * @property {string} [error]
   * @property {boolean} [busy]
   * @property {() => void} onmove
   * @property {() => void} onkeepseparate
   */

  /** @type {Props} */
  let { pendingMove, error = "", busy = false, onmove, onkeepseparate } = $props();

  /** @type {HTMLDialogElement | undefined} */
  let dialog = $state();

  $effect(() => {
    if (!dialog) return;
    if (pendingMove && !dialog.open) dialog.showModal();
    if (!pendingMove && dialog.open) dialog.close();
  });
</script>

<Dialog bind:element={dialog} oncancel={(event) => event.preventDefault()}>
  {#if pendingMove}
    <Heading>
      {ngettext(
        msgid`Move ${pendingMove.items} item into ${pendingMove.email}?`,
        `Move ${pendingMove.items} items into ${pendingMove.email}?`,
        pendingMove.items,
      )}
    </Heading>
    {#snippet strongEmail()}<strong>{pendingMove.email}</strong>{/snippet}
    <Text>
      <TranslatedParts parts={jt`What you made on this device becomes part of ${strongEmail} and syncs with it.`} />
      {t`The account that was only on this device is gone afterwards.`}
    </Text>
    <Text tone="muted" size="sm">
      {t`Keeping them separate adds ${pendingMove.email} beside it, and leaves what you made where it is, on this device only.`}
    </Text>
    <ErrorMessage message={error} />
    <DialogActions>
      <Button size="sm" variant="ghost" onclick={onkeepseparate} disabled={busy}>{t`Keep them separate`}</Button>
      <Button size="sm" variant="primary" onclick={onmove} disabled={busy} data-move>{t`Move`}</Button>
    </DialogActions>
  {/if}
</Dialog>
