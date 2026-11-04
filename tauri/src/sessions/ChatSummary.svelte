<!-- What a chat holds (ChatSummary in claude_code_chat.rs), laid out like the
     Git menu's breakdown, and where it was read from. -->
<script>
  import { countOf, timeAgo } from "./formatting.js";

  let { chat } = $props();

  let summary = $derived(chat.summary);
  let counts = $derived([
    ["Your prompts", summary.prompts],
    ["Agent messages", summary.assistant_messages],
    ["Thinking", summary.thinking_blocks],
    ["Tool calls", summary.tool_calls],
  ]);

  function timeOf(timestamp) {
    if (!timestamp) return "unknown";
    return `${new Date(timestamp).toLocaleString()} (${timeAgo(Date.parse(timestamp))})`;
  }
</script>

<p class="git-summary">{countOf(chat.messages.length, "message")}</p>
<dl class="git-details">
  {#each counts as [label, count] (label)}
    <dt>{label}</dt>
    <dd>{count}</dd>
  {/each}
  <dt class={{ "chat-tool-error": summary.failed_tool_calls > 0 }}>Failed tool calls</dt>
  <dd>{summary.failed_tool_calls}</dd>
  <dt>Without a result yet</dt>
  <dd>{summary.tool_calls_without_result}</dd>
  <dt>Started</dt>
  <dd>{timeOf(summary.started_at)}</dd>
  <dt>Last message</dt>
  <dd>{timeOf(summary.last_message_at)}</dd>
</dl>
{#if summary.tool_calls_by_name.length > 0}
  <p class="git-summary mt-3">Tool calls by tool</p>
  <dl class="git-details">
    {#each summary.tool_calls_by_name as { name, count } (name)}
      <dt>{name}</dt>
      <dd>{count}</dd>
    {/each}
  </dl>
{/if}
<p class="git-hint">
  Read from <code class="chat-path">{chat.path}</code>, the transcript Claude Code writes as it works. It isn't documented,
  so lines it can't read are left out, and long tool inputs and results are cut short.
</p>
