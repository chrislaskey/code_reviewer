<script>
  import ChoiceMenu from "../lib/components/ChoiceMenu.svelte";
  import MenuChoice from "../lib/components/MenuChoice.svelte";

  let { name, port, onOpen, onOpenInBrowser, onCopy } = $props();

  let attachCommand = $derived(`tmux attach -t ${name}`);
  let attachCommandElement;
</script>

<ChoiceMenu label="Open" title="Get to the session">
  {#snippet children({ close })}
    {#if port}
      <MenuChoice
        label="Open in browser"
        explanation="Opens http://localhost:{port} in the web browser."
        onclick={() => {
          close();
          onOpenInBrowser(name);
        }}
      />
    {/if}
    <MenuChoice
      label="Open in the terminal app"
      explanation="Brings the session's window to the front, or opens a new window attached to it."
      onclick={() => {
        close();
        onOpen(name);
      }}
    />
    <MenuChoice
      label="Copy attach command"
      explanation="To paste into any terminal:"
      onclick={async () => {
        // When copying fails the command is selected, so the menu stays open.
        if (await onCopy(attachCommand, attachCommandElement)) close();
      }}
    >
      <code class="attach-command" bind:this={attachCommandElement}>{attachCommand}</code>
    </MenuChoice>
  {/snippet}
</ChoiceMenu>
