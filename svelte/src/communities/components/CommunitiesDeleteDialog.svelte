<!-- Asked before a community is deleted: what a delete takes with it. -->
<script>
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
    <Heading>Delete {community.name}?</Heading>
    <Text>
      Every member leaves it, and the posts in it are taken out of it. The posts themselves stay with the people who
      wrote them.
    </Text>
    <DialogActions>
      <Button size="sm" variant="ghost" onclick={oncancel} disabled={busy}>Keep</Button>
      <Button size="sm" variant="danger" onclick={ondelete} disabled={busy}>Delete community</Button>
    </DialogActions>
  {/if}
</Dialog>
