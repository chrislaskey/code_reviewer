<!-- What went wrong, when something did: nothing while `message` is empty. -->
<script>
  /** @type {{ message: string | null | undefined, size?: "md" | "sm", class?: any }} */
  let { message, size = "md", class: class_ } = $props();
</script>

{#if message}
  <p class={["error", size === "sm" && "text-sm", class_]} role="alert">{message}</p>
{/if}
