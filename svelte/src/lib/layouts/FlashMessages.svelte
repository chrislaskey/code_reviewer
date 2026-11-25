<!-- The flash messages above the page (flash_messages.svelte.js), each a
     daisyUI alert in its tone, with its icon and a close button. A screen
     reader announces an error at once, and the others when it is idle. -->
<script>
  import { t } from "ttag";
  import { flashMessageClose, flashMessages } from "../flash_messages.svelte.js";
  import { Alert, Button, Icon } from "../components/index.js";
</script>

{#if flashMessages.value.length > 0}
  <div class="app-banner flex flex-col gap-2" data-flash-messages>
    {#each flashMessages.value as message (message.id)}
      <Alert tone={message.tone} role={message.tone === "error" ? "alert" : "status"}>
        {#if message.icon}
          <Icon name={message.icon} class="size-5" />
        {/if}
        <span>{message.text}</span>
        <Button variant="ghost" size="xs" class="btn-square" aria-label={t`Close`} title={t`Close`} onclick={() => flashMessageClose(message.id)}>
          <Icon name="hero-x-mark" />
        </Button>
      </Alert>
    {/each}
  </div>
{/if}
