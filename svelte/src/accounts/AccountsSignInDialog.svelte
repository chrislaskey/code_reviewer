<!-- Shown while a sign-in waits on the browser, inside the app: the
     browser has the web's sign-in page and ends by showing a code, which
     the person pastes here, see docs/design/accounts-sign-in.md. -->
<script>
  import { Button, ErrorMessage, Heading, Input, Text } from "../lib/components/index.js";
  import Dialog from "../lib/Dialog.svelte";
  import DialogActions from "../lib/DialogActions.svelte";
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
    <Heading>Finish signing in in your browser</Heading>
    <Text>
      {#if pending.email}
        The sign-in page for <strong>{pending.email}</strong> opened in your browser.
      {:else}
        The sign-in page opened in your browser.
      {/if}
      When it shows a code, paste it here.
    </Text>
    <form onsubmit={submit} class="flex flex-col gap-3">
      <Input bind:value={code} autocomplete="off" aria-label="Code from the browser" placeholder="Code from the browser" required />
      <ErrorMessage message={error} />
      <DialogActions>
        <Button size="sm" variant="ghost" onclick={oncancel} disabled={busy}>Cancel</Button>
        <Button size="sm" onclick={onopenbrowser} disabled={busy}>Open the browser again</Button>
        <Button size="sm" variant="primary" type="submit" disabled={busy}>Finish</Button>
      </DialogActions>
    </form>
  {/if}
</Dialog>
