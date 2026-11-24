<!-- One message of an agent's chat (ChatMessage in agent_chat.rs):
     the user's prompt or the assistant's reply, with its thinking and its
     tool calls folded, each call's result inside it, or a note the agent's
     CLI wrote, folded to one line. -->
<script>
  import { AGENT_NAMES, timeLater } from "./formatting.js";

  /**
   * `previousTimestamp`: the message before's, to say how long after it this one came. `agent`: who wrote the
   * chat, naming its notes. `hasToolResults`: false when the transcript leaves them out, as Cursor's does.
   */
  let { message, previousTimestamp = null, agent, hasToolResults = true } = $props();

  let roleLabels = $derived({ user: "You", assistant: "Agent", note: AGENT_NAMES[agent] ?? "Agent CLI" });
  const NOTE_LABELS = {
    context: "Context",
    compact_summary: "Conversation summary",
    command: "Command",
    command_output: "Command output",
    task_notification: "Background task",
    interrupted: "Interrupted",
    other: "Note",
  };

  let time = $derived(message.timestamp ? new Date(message.timestamp).toLocaleTimeString() : "");
  let later = $derived(timeLater(previousTimestamp, message.timestamp));
</script>

{#snippet toolText(toolText, isError = false)}
  <pre class="chat-tool-text" class:chat-tool-error={isError}>{toolText.text}</pre>
  {#if toolText.truncated}
    <p class="muted text-sm">Cut short, the rest is in the transcript.</p>
  {/if}
{/snippet}

{#snippet header()}
  <span class="chat-role">{roleLabels[message.role] ?? message.role}</span>
  <time class="muted" datetime={message.timestamp}>{time}</time>
  {#if later}
    <span class="muted">{later}</span>
  {/if}
{/snippet}

{#if message.note}
  <details class="chat-note chat-fold">
    <summary>
      {@render header()}
      <span class="chat-note-kind">{NOTE_LABELS[message.note.kind] ?? message.note.kind}</span>
      <span class="chat-tool-summary">{message.note.summary}</span>
    </summary>
    {@render parts()}
  </details>
{:else}
  <article class="chat-message chat-message-{message.role}">
    <header class="chat-message-header">{@render header()}</header>
    {@render parts()}
  </article>
{/if}

{#snippet parts()}
  {#each message.parts as part, index (index)}
    {#if part.type === "text"}
      <div class="markdown-html chat-text">{@html part.html}</div>
    {:else if part.type === "plain_text"}
      {@render toolText(part.text)}
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
          {#if !part.result && hasToolResults}
            <span class="chat-tool-status muted">· no result yet</span>
          {:else if part.result?.is_error}
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
{/snippet}
