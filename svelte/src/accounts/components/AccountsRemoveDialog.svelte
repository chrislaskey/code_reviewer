<!-- Asked before an account is removed from this device: what removal
     means, and, when changes would be lost, how many and the choice to
     remove anyway. A local user's is the only copy, and says so. -->
<script>
  import { conflictsIndexUrl } from "../../lib/routes.js";
  import { Button, Dialog, DialogActions, Heading, Text } from "../../lib/components/index.js";

  /**
   * @typedef {Object} Props
   * @property {{email: string, active: boolean, local?: boolean} | null} account the account to remove; the dialog is open while set
   * @property {{pending: number, conflicts: number} | null} loss what a removal would lose, once the first attempt said so
   * @property {boolean} [busy]
   * @property {(losingChanges: boolean) => void} onremove
   * @property {() => void} oncancel
   */

  /** @type {Props} */
  let { account, loss, busy = false, onremove, oncancel } = $props();

  /** @type {HTMLDialogElement | undefined} */
  let dialog = $state();

  $effect(() => {
    if (!dialog) return;
    if (account && !dialog.open) dialog.showModal();
    if (!account && dialog.open) dialog.close();
  });

  /** @param {number} count @param {string} one @param {string} many */
  const counted = (count, one, many) => `${count} ${count === 1 ? one : many}`;
</script>

<Dialog bind:element={dialog} onclose={oncancel}>
  {#if account}
    <Heading>Remove {account.local ? "the account that is only on this device" : account.email}?</Heading>
    {#if account.local}
      <Text>
        <strong>This is the only copy.</strong> Everything in this account is on this device alone and is deleted
        with it. This cannot be undone. To keep it, sign up or sign in first and move it into your account.
      </Text>
    {:else if loss}
      <ul class="list-disc pl-5">
        {#if loss.pending > 0}
          <li>{counted(loss.pending, "change has", "changes have")} not reached the server yet.</li>
        {/if}
        {#if loss.conflicts > 0}
          <li>{counted(loss.conflicts, "change was", "changes were")} not kept by the server.</li>
        {/if}
      </ul>
      <Text>Removing the account drops them from this device.</Text>
    {:else}
      <Text>
        Any data stored on this device for this account is removed. What was synced stays on the server and on other
        devices, and signing in again later downloads it to this device again.
      </Text>
    {/if}
    <DialogActions>
      {#if loss && account.active}
        <Button size="sm" href={conflictsIndexUrl()} onclick={oncancel}>View changes</Button>
      {/if}
      <Button size="sm" variant="ghost" onclick={oncancel} disabled={busy}>Keep</Button>
      <Button size="sm" variant="danger" onclick={() => onremove(loss !== null)} disabled={busy}>
        {account.local ? "Delete it all" : loss ? "Remove anyway" : "Remove from device"}
      </Button>
    </DialogActions>
  {/if}
</Dialog>
