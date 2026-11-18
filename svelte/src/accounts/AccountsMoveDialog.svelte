<!-- Asked after a sign-in on a device that has a local user: whether
     what they made moves into the account that just signed in, or the
     two stay apart, see docs/design/accounts-local-and-trial-users.md. -->
<script>
  import { Button, ErrorMessage, Heading, Text } from "../lib/components/index.js";
  import Dialog from "../lib/Dialog.svelte";
  import DialogActions from "../lib/DialogActions.svelte";
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
    <Heading>Move {offered.items} {offered.items === 1 ? "item" : "items"} into {offered.email}?</Heading>
    <Text>
      What you made on this device becomes part of <strong>{offered.email}</strong> and syncs with it. The account
      that was only on this device is gone afterwards.
    </Text>
    <Text tone="muted" size="sm">
      Keeping them separate adds {offered.email} beside it, and leaves what you made where it is, on this device
      only.
    </Text>
    <ErrorMessage message={error} />
    <DialogActions>
      <Button size="sm" variant="ghost" onclick={onkeepseparate} disabled={busy}>Keep them separate</Button>
      <Button size="sm" variant="primary" onclick={onmove} disabled={busy} data-move>Move</Button>
    </DialogActions>
  {/if}
</Dialog>
