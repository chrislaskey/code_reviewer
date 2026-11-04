<!-- One message of an agent's chat (ChatMessage in claude_code_chat.rs):
     the user's prompt or the assistant's reply, with its thinking and its
     tool calls folded, each call's result inside it. -->
<script>
  import { timeLater } from "./formatting.js";

  /** `previousTimestamp`: the message before's, to say how long after it this one came. */
  let { message, previousTimestamp = null } = $props();

  const ROLE_LABELS = { user: "You", assistant: "Agent" };

  let time = $derived(message.timestamp ? new Date(message.timestamp).toLocaleTimeString() : "");
  let later = $derived(timeLater(previousTimestamp, message.timestamp));
</script>

{#snippet toolText(toolText, isError = false)}
  <pre class="chat-tool-text" class:chat-tool-error={isError}>{toolText.text}</pre>
  {#if toolText.truncated}
    <p class="muted text-sm">Cut short, the rest is in the transcript.</p>
  {/if}
{/snippet}

<article class="chat-message chat-message-{message.role}">
  <header class="chat-message-header">
    <span class="chat-role">{ROLE_LABELS[message.role] ?? message.role}</span>
    <time class="muted" datetime={message.timestamp}>{time}</time>
    {#if later}
      <span class="muted">{later}</span>
    {/if}
  </header>
  {#each message.parts as part, index (index)}
    {#if part.type === "text"}
      <div class="markdown-html chat-text">{@html part.html}</div>
    {:else if part.type === "thinking"}
      <details class="chat-fold">
        <summary>Thinking</summary>
        <p class="chat-thinking">{part.text}</p>
      </details>
    {:else if part.type === "tool_use"}
      <details class="chat-fold">
        <summary>
          <span class="chat-tool-name">{part.name}</span>
          <span class="chat-tool-summary">{part.summary}</span>
          {#if !part.result}
            <span class="chat-tool-status muted">· no result yet</span>
          {:else if part.result.is_error}
            <span class="chat-tool-status chat-tool-error">· failed</span>
          {/if}
        </summary>
        <p class="chat-fold-label">Input</p>
        {@render toolText(part.input)}
        {#if part.result}
          <p class="chat-fold-label">Result</p>
          {@render toolText(part.result.output, part.result.is_error)}
        {/if}
      </details>
    {:else if part.type === "tool_result"}
      <details class="chat-fold">
        <summary>Tool result</summary>
        {@render toolText(part.result.output, part.result.is_error)}
      </details>
    {/if}
  {/each}
</article>
