<!-- A box to write a prompt to a session's agent. Nothing reaches the
     session until Enter or Send; Shift+Enter starts a new line. `onSend`
     resolves to whether it was sent, and the text is kept when it wasn't. -->
<script>
  import Button from "../lib/components/Button.svelte";

  /** `working`: the agent is busy, so the box glows as the status bar does. */
  /** @type {{ disabled?: boolean, disabledReason?: string, working?: boolean, onSend: (prompt: string) => Promise<boolean> }} */
  let { disabled = false, disabledReason = "", working = false, onSend } = $props();

  let prompt = $state("");
  let sending = $state(false);

  let canSend = $derived(!disabled && !sending && prompt.trim() !== "");

  async function send(event) {
    event?.preventDefault();
    if (!canSend) return;
    sending = true;
    try {
      if (await onSend(prompt)) prompt = "";
    } finally {
      sending = false;
    }
  }

  function sendOnEnter(event) {
    // isComposing: Enter that picks a character in an input method.
    if (event.key === "Enter" && !event.shiftKey && !event.isComposing) send(event);
  }
</script>

<form class="prompt-form panel" class:working onsubmit={send}>
  <textarea
    class="textarea prompt-input w-full"
    rows="3"
    placeholder={disabled ? disabledReason : "Write to the agent. Enter sends, Shift+Enter starts a new line."}
    bind:value={prompt}
    onkeydown={sendOnEnter}
    {disabled}
  ></textarea>
  <div class="prompt-actions">
    <span class="muted text-sm">Typed into the agent's tmux pane, then Enter.</span>
    <Button type="submit" variant="primary" disabled={!canSend}>{sending ? "Sending…" : "Send"}</Button>
  </div>
</form>
