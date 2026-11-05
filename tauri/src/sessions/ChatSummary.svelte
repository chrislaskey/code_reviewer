<!-- What a chat holds (ChatSummary in agent_chat.rs), laid out like the
     Git menu's breakdown, and where it was read from. -->
<script>
  import { AGENT_NAMES, countOf, timeAgo } from "./formatting.js";

  let { chat } = $props();

  let summary = $derived(chat.summary);
  let agentName = $derived(AGENT_NAMES[chat.agent] ?? "The agent");
  let counts = $derived([
    ["Your prompts", summary.prompts],
    ["Agent messages", summary.assistant_messages],
    [`${agentName} notes`, summary.notes],
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
  {#if chat.has_tool_results}
    <dt class={{ "chat-tool-error": summary.failed_tool_calls > 0 }}>Failed tool calls</dt>
    <dd>{summary.failed_tool_calls}</dd>
    <dt>Without a result yet</dt>
    <dd>{summary.tool_calls_without_result}</dd>
  {/if}
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
  {agentName} notes are lines it writes as yours, like context it adds and commands you run. All shows them folded.
</p>
{#if !chat.has_tool_results}
  <p class="git-hint">
    {agentName}'s transcript leaves out tool results, and only your prompts have a time.
  </p>
{/if}
<p class="git-hint">
  Read from <code class="chat-path">{chat.path}</code>, the transcript {agentName} writes as it works. It isn't documented,
  so lines it can't read are left out, and long tool inputs and results are cut short.
</p>
