<!-- What a chat holds (ChatSummary in agent_chat.rs), laid out like the
     Git menu's breakdown, and where it was read from. -->
<script>
  import { jt, msgid, ngettext, t } from "ttag";
  import TranslatedParts from "../lib/components/TranslatedParts.svelte";
  import { AGENT_NAMES, contextUsageText, timeAgo } from "./formatting.js";

  let { chat } = $props();

  let summary = $derived(chat.summary);
  let agentName = $derived(AGENT_NAMES[chat.agent] ?? t`The agent`);
  let messageCount = $derived(chat.messages.length);
  let counts = $derived([
    [t`Your prompts`, summary.prompts],
    [t`Agent messages`, summary.assistant_messages],
    [t`${agentName} notes`, summary.notes],
    [t`Thinking`, summary.thinking_blocks],
    [t`Tool calls`, summary.tool_calls],
  ]);

  function timeOf(timestamp) {
    if (!timestamp) return t`unknown`;
    return `${new Date(timestamp).toLocaleString()} (${timeAgo(Date.parse(timestamp))})`;
  }
</script>

{#snippet transcriptPath()}<code class="chat-path">{chat.path}</code>{/snippet}

<p class="git-summary">{ngettext(msgid`${messageCount} message`, `${messageCount} messages`, messageCount)}</p>
<dl class="git-details">
  {#each counts as [label, count] (label)}
    <dt>{label}</dt>
    <dd>{count}</dd>
  {/each}
  {#if chat.has_tool_results}
    <dt class={{ "chat-tool-error": summary.failed_tool_calls > 0 }}>{t`Failed tool calls`}</dt>
    <dd>{summary.failed_tool_calls}</dd>
    <dt>{t`Without a result yet`}</dt>
    <dd>{summary.tool_calls_without_result}</dd>
  {/if}
  {#if chat.context_usage}
    <dt>{t`Context window`}</dt>
    <dd>{contextUsageText(chat.context_usage)}</dd>
  {/if}
  <dt>{t`Started`}</dt>
  <dd>{timeOf(summary.started_at)}</dd>
  <dt>{t`Last message`}</dt>
  <dd>{timeOf(summary.last_message_at)}</dd>
</dl>
{#if summary.tool_calls_by_name.length > 0}
  <p class="git-summary mt-3">{t`Tool calls by tool`}</p>
  <dl class="git-details">
    {#each summary.tool_calls_by_name as { name, count } (name)}
      <dt>{name}</dt>
      <dd>{count}</dd>
    {/each}
  </dl>
{/if}
<p class="git-hint">
  {t`${agentName} notes are lines it writes as yours, like context it adds and commands you run. All shows them folded.`}
</p>
{#if !chat.has_tool_results}
  <p class="git-hint">
    {t`${agentName}'s transcript leaves out tool results, and only your prompts have a time.`}
  </p>
{/if}
<p class="git-hint">
  <TranslatedParts
    parts={jt`Read from ${transcriptPath}, the transcript ${agentName} writes as it works. It isn't documented, so lines it can't read are left out, and long tool inputs and results are cut short.`}
  />
</p>
