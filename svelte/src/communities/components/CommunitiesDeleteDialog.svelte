<!-- Asked before a community is deleted: what a delete takes with it. -->
<script>
  import { t } from "ttag";
  import { Button, Dialog, DialogActions, Heading, Text } from "../../lib/components/index.js";
  /**
   * @typedef {Object} Props
   * @property {{name: string} | null} community the community to delete; the dialog is open while set
   * @property {boolean} [busy]
   * @property {() => void} ondelete
   * @property {() => void} oncancel
   */

  /** @type {Props} */
  let { community, busy = false, ondelete, oncancel } = $props();

  /** @type {HTMLDialogElement | undefined} */
  let dialog = $state();

  $effect(() => {
    if (!dialog) return;
    if (community && !dialog.open) dialog.showModal();
    if (!community && dialog.open) dialog.close();
  });
</script>

<Dialog bind:element={dialog} onclose={oncancel}>
  {#if community}
    <Heading>{t`Delete ${community.name}?`}</Heading>
    <Text>
      {t`Every member leaves it, and everything in it is taken out of it.`}
    </Text>
    <DialogActions>
      <Button size="sm" variant="ghost" onclick={oncancel} disabled={busy}>{t`Keep`}</Button>
      <Button size="sm" variant="danger" onclick={ondelete} disabled={busy}>{t`Delete community`}</Button>
    </DialogActions>
  {/if}
</Dialog>
