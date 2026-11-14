<!-- A box to write a prompt to a session's agent. Nothing reaches the
     session until Enter or Send; Shift+Enter starts a new line. `onSend`
     resolves to whether it was sent, and the text is kept when it wasn't. With
     `draftKey`, the unsent text is kept in this client's localStorage under it.
     With `onAttachImage`, images pasted, dropped or picked are saved by it and
     their paths put in the prompt where the cursor is. -->
<script>
  import Button from "../lib/components/Button.svelte";
  import InfoPopover from "../lib/components/InfoPopover.svelte";
  import Icon from "../lib/components/Icon.svelte";
  import { loadPromptDraft, savePromptDraft } from "./prompt_draft.js";
  import { imageFilesIn, insertIntoPrompt } from "./prompt_images.js";

  /** `controls`: shown on the left of the Send row, like the chat's toggles. */
  /** `onAttachImage`: saves an image and resolves to its path, or `null` when it couldn't. */
  /** @type {{ draftKey?: string, disabled?: boolean, disabledReason?: string, onSend: (prompt: string) => Promise<boolean>, onAttachImage?: (file: File) => Promise<string | null>, controls?: import("svelte").Snippet<[]> }} */
  let { draftKey, disabled = false, disabledReason = "", onSend, onAttachImage, controls } = $props();

  // The page is remounted per session, so the key only matters at the start.
  // svelte-ignore state_referenced_locally
  let prompt = $state(draftKey ? loadPromptDraft(draftKey) : "");
  let sending = $state(false);
  let attachingCount = $state(0);
  /** @type {HTMLTextAreaElement | undefined} */
  let textarea = $state();
  /** @type {HTMLInputElement | undefined} */
  let imagePicker = $state();

  $effect(() => {
    if (draftKey) savePromptDraft(draftKey, prompt);
  });

  let canSend = $derived(!disabled && !sending && attachingCount === 0 && prompt.trim() !== "");

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

  async function attachImages(files) {
    if (!onAttachImage || files.length === 0) return;
    attachingCount += files.length;
    for (const file of files) {
      try {
        const path = await onAttachImage(file);
        if (path) insertAtCursor(path);
      } finally {
        attachingCount -= 1;
      }
    }
  }

  function insertAtCursor(text) {
    const position = textarea?.selectionEnd ?? prompt.length;
    const inserted = insertIntoPrompt(prompt, position, text);
    prompt = inserted.prompt;
    queueMicrotask(() => textarea?.setSelectionRange(inserted.position, inserted.position));
  }

  function attachPastedImages(event) {
    const files = imageFilesIn(event.clipboardData);
    if (!onAttachImage || files.length === 0) return;
    event.preventDefault();
    attachImages(files);
  }

  function attachDroppedImages(event) {
    const files = imageFilesIn(event.dataTransfer);
    if (!onAttachImage || files.length === 0) return;
    event.preventDefault();
    attachImages(files);
  }

  function attachPickedImages(event) {
    attachImages(imageFilesIn(event.currentTarget));
    event.currentTarget.value = "";
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
    bind:this={textarea}
    onkeydown={sendOnEnter}
    onpaste={attachPastedImages}
    ondrop={attachDroppedImages}
    {disabled}
  ></textarea>
  <div class="prompt-actions">
    {@render controls?.()}
    {#if onAttachImage}
      <input
        class="hidden"
        type="file"
        accept="image/png,image/jpeg,image/gif,image/webp"
        multiple
        bind:this={imagePicker}
        onchange={attachPickedImages}
      />
      <Button
        type="button"
        class="btn btn-ghost btn-sm"
        title="Attach an image: its path is put in the prompt. You can also paste or drop one."
        disabled={disabled || sending}
        onclick={() => imagePicker?.click()}
      >
        <Icon name="hero-photo-micro" class="size-4" />
        {attachingCount > 0 ? "Attaching…" : "Image"}
      </Button>
    {/if}
    <InfoPopover title="How the prompt is sent" class="prompt-hint dropdown-top dropdown-end">
      Typed into the agent's tmux pane, then Enter, as if you typed it there. Nothing is sent until you press Enter or
      Send, and Shift+Enter starts a new line. A pasted, dropped or attached image is saved in the session's artifacts
      and its path put in the prompt, for the agent to read.
    </InfoPopover>
    <Button type="submit" variant="primary" disabled={!canSend}>{sending ? "Sending…" : "Send"}</Button>
  </div>
</form>
