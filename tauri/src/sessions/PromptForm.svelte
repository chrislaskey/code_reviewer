<!-- A box to write a prompt to a session's agent. Nothing reaches the
     session until Enter or Send; Shift+Enter starts a new line. `onSend`
     resolves to whether it was sent, and the text is kept when it wasn't. With
     `draftKey`, the unsent text is kept in this client's localStorage under it. -->
<script>
  import Button from "../lib/components/Button.svelte";
  import InfoPopover from "../lib/components/InfoPopover.svelte";
  import { loadPromptDraft, savePromptDraft } from "./prompt_draft.js";

  /** `controls`: shown on the left of the Send row, like the git status. */
  /** @type {{ draftKey?: string, disabled?: boolean, disabledReason?: string, onSend: (prompt: string) => Promise<boolean>, controls?: import("svelte").Snippet<[]> }} */
  let { draftKey, disabled = false, disabledReason = "", onSend, controls } = $props();

  // The page is remounted per session, so the key only matters at the start.
  // svelte-ignore state_referenced_locally
  let prompt = $state(draftKey ? loadPromptDraft(draftKey) : "");
  let sending = $state(false);

  $effect(() => {
    if (draftKey) savePromptDraft(draftKey, prompt);
  });

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

<form class="prompt-form panel" onsubmit={send}>
  <textarea
    class="textarea prompt-input w-full"
    rows="3"
    placeholder={disabled ? disabledReason : "Write to the agent. Enter sends, Shift+Enter starts a new line."}
    bind:value={prompt}
    onkeydown={sendOnEnter}
    {disabled}
  ></textarea>
  <div class="prompt-actions">
    {@render controls?.()}
    <InfoPopover title="How the prompt is sent" class="prompt-hint dropdown-top dropdown-end">
      Typed into the agent's tmux pane, then Enter, as if you typed it there. Nothing is sent until you press Enter or
      Send, and Shift+Enter starts a new line.
    </InfoPopover>
    <Button type="submit" variant="primary" disabled={!canSend}>{sending ? "Sending…" : "Send"}</Button>
  </div>
</form>
