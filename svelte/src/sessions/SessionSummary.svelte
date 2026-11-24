<!-- A tmux session on the sessions page: its name, repository, bell and
     agent status, glowing while the agent works. The whole card opens the
     session's page, where its actions are. -->
<script>
  import { t } from "ttag";
  import Icon from "../lib/components/Icon.svelte";
  import { sessionsShowUrl } from "../lib/routes.js";
  import AgentBadge from "./AgentBadge.svelte";
  import Badge from "./Badge.svelte";
  import NotificationsEnabledIcon from "./NotificationsEnabledIcon.svelte";

  let { session } = $props();

  let working = $derived(session.agent?.status === "busy");
</script>

<a class="raised-panel session-link" class:working href={sessionsShowUrl(session.name)}>
  <div class="session-header">
    <span class="session-title">
      <span class="name">{session.name}</span>
      {#if session.repository_name}<span class="repository-name">{session.repository_name}</span>{/if}
    </span>
    {#if session.notifications_enabled}<NotificationsEnabledIcon />{/if}
    {#if session.agent}
      <AgentBadge session={session.agent} />
    {:else}
      <Badge text={t`no agent`} title={t`No agent is running in the session`} />
    {/if}
    <Icon name="hero-chevron-right-micro" class="session-link-arrow size-4" />
  </div>
</a>
