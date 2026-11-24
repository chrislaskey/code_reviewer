<script>
  import { t } from "ttag";
  import ChoiceMenu from "../lib/code_reviewer/components/ChoiceMenu.svelte";
  import MenuChoice from "../lib/code_reviewer/components/MenuChoice.svelte";

  let { name, port, onOpen, onOpenInBrowser, onOpenArtifacts, onCopy, class: class_ = undefined } = $props();

  // A phone has no terminal app of the server's to bring forward.
  const onPhone = document.documentElement.dataset.app === "mobile";

  let attachCommand = $derived(`tmux attach -t ${name}`);
  let attachCommandElement;
</script>

<ChoiceMenu label={t`Open`} title={t`Get to the session`} class={class_}>
  {#snippet children({ close })}
    {#if port}
      <MenuChoice
        label={t`Open in browser`}
        explanation={t`Opens http://localhost:${port} in the web browser, at the computer's tailnet name on a phone.`}
        onclick={() => {
          close();
          onOpenInBrowser(name);
        }}
      />
    {/if}
    <MenuChoice
      label={t`Open artifacts`}
      explanation={t`Opens the screenshots, previews and other files made in the session's checkout in the web browser. Needs bin/dev running in the main checkout.`}
      onclick={() => {
        close();
        onOpenArtifacts(name);
      }}
    />
    {#if !onPhone}
      <MenuChoice
        label={t`Open in the terminal app`}
        explanation={t`Brings the session's window to the front, or opens a new window attached to it.`}
        onclick={() => {
          close();
          onOpen(name);
        }}
      />
    {/if}
    <MenuChoice
      label={t`Copy attach command`}
      explanation={t`To paste into any terminal:`}
      onclick={async () => {
        // When copying fails the command is selected, so the menu stays open.
        if (await onCopy(attachCommand, attachCommandElement)) close();
      }}
    >
      <code class="attach-command" bind:this={attachCommandElement}>{attachCommand}</code>
    </MenuChoice>
  {/snippet}
</ChoiceMenu>
