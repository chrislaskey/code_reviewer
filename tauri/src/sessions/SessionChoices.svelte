<!-- A menu's choices of session, to switch between them: each of the app's
     tmux sessions with its repository and agent status, from the cached
     list the sessions page shows, asked for again while the menu is open.
     A choice opens the session's page, then calls `onchoose`. -->
<script>
  import { onMount } from "svelte";
  import { listSessionSummaries } from "../lib/backend.js";
  import { repeatWhileVisible } from "../lib/repeat_while_visible.js";
  import { sessionNameFromPath, sessionShowUrl } from "../lib/routes.js";
  import AgentBadge from "./AgentBadge.svelte";
  import Badge from "./Badge.svelte";

  const REFRESH_INTERVAL_MS = 2000;

  /**
   * @typedef {Object} Props
   * @property {() => void} [onchoose]
   */

  /** @type {Props} */
  let { onchoose } = $props();

  let sessions = $state(null);
  let error = $state(null);
  const currentName = sessionNameFromPath(location.pathname);

  async function refresh() {
    try {
      sessions = (await listSessionSummaries()).sessions;
      error = null;
    } catch (caught) {
      error = String(caught);
    }
  }

  onMount(() => {
    refresh();
    return repeatWhileVisible(refresh, REFRESH_INTERVAL_MS);
  });
</script>

{#if error && !sessions}
  <li class="menu-disabled"><span class="text-error">{error}</span></li>
{:else if sessions?.length === 0}
  <li class="menu-disabled"><span>No sessions</span></li>
{:else if sessions}
  {#each sessions as session (session.name)}
    <li>
      <a href={sessionShowUrl(session.name)} class="session-choice" class:menu-active={session.name === currentName} onclick={onchoose}>
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
