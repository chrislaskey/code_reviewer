<!-- A menu's choices of session, to switch between them: each of the app's
     tmux sessions with its repository, bell and agent status, from the cached
     list the sessions page shows, asked for again while the menu is open.
     Nothing shows until there are sessions, an error included; `sessions`
     is bindable so the menu can show its heading only then. A choice opens
     the session's page, then calls `onchoose`. -->
<script>
  import { onMount } from "svelte";
  import { sessionsList } from "../lib/core/index.js";
  import { repeatWhileVisible } from "../lib/code_reviewer/repeat_while_visible.js";
  import { sessionsShowUrl } from "../lib/routes.js";
  import AgentBadge from "./AgentBadge.svelte";
  import Badge from "./Badge.svelte";

  const REFRESH_INTERVAL_MS = 2000;

  /**
   * @typedef {Object} Props
   * @property {object[] | null} [sessions]
   * @property {() => void} [onchoose]
   */

  /** @type {Props} */
  let { sessions = $bindable(null), onchoose } = $props();

  async function refresh() {
    try {
      sessions = (await sessionsList()).sessions;
    } catch (error) {
      console.error("Listing sessions for the menu failed:", error);
    }
  }

  onMount(() => {
    refresh();
    return repeatWhileVisible(refresh, REFRESH_INTERVAL_MS);
  });
</script>

{#if sessions}
  {#each sessions as session (session.name)}
    <li>
      <a href={sessionsShowUrl(session.name)} class="session-choice text-base-content no-underline" onclick={onchoose}>
        <span class="session-choice-name">
          {session.name}
          {#if session.repository_name}<span class="session-choice-repository">{session.repository_name}</span>{/if}
        </span>
        {#if session.agent}
          <AgentBadge session={session.agent} />
        {:else}
          <Badge text="no agent" title="No agent is running in the session" />
        {/if}
      </a>
    </li>
  {/each}
{/if}

<style>
  .session-choice {
    display: flex;
    align-items: center;
    gap: 0.5rem;
  }

  .session-choice-name {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .session-choice-repository {
    margin-left: 0.375rem;
    color: var(--muted);
    font-size: 0.8rem;
  }
</style>
