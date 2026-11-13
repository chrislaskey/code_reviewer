<!-- A tmux session's status bar: its name and repository, the agent's
     status, and the Open, Review, Trace and Git menus, which move to a
     second line of smaller buttons on small windows. On the sessions page
     and on a session's own page, which passes `showDetailsLink={false}`.
     `actions` comes from createSessionActions (session_actions.js). -->
<script>
  import { reviewShowUrl, sessionShowUrl, traceShowUrl } from "../lib/routes.js";
  import AgentBadge from "./AgentBadge.svelte";
  import Badge from "./Badge.svelte";
  import ChangesMenu from "./ChangesMenu.svelte";
  import GitMenu from "./GitMenu.svelte";
  import OpenMenu from "./OpenMenu.svelte";
  import RepositoryName from "./RepositoryName.svelte";

  let { session, mainBranch, actions, showDetailsLink = true } = $props();

  const ACTION_BUTTON_CLASS = "btn btn-ghost max-sm:btn-sm";

  let attached = $derived(session.attached_clients > 0);
  let detailsUrl = $derived(showDetailsLink && attached && session.agent ? sessionShowUrl(session.name) : null);
</script>

<div class="session-header">
  <span class="session-title">
    {#if detailsUrl}
      <a class="name" href={detailsUrl}>{session.name}</a>
    {:else}
      <span class="name">{session.name}</span>
    {/if}
    <RepositoryName path={session.directory ?? ""} git={session.git} />
  </span>
  {#if session.agent}
    <AgentBadge session={session.agent} />
  {:else}
    <Badge text="no agent" title="No agent is running in the session" />
  {/if}
  {#if !attached}
    <Badge text="detached" title="No terminal window is attached to the session, so Open starts a new one" />
  {/if}
  <div class="session-actions">
    <OpenMenu
      name={session.name}
      port={session.port}
      {detailsUrl}
      onOpen={actions.showInTerminalApp}
      onOpenInBrowser={actions.openInBrowser}
      onOpenArtifacts={actions.openArtifactsInBrowser}
      onCopy={actions.copyText}
      class={ACTION_BUTTON_CLASS}
    />
    {#if session.directory}
      <ChangesMenu label="Review" title="Open the changes in Reviews" directory={session.directory} git={session.git} {mainBranch} urlFor={reviewShowUrl} class={ACTION_BUTTON_CLASS} />
      <ChangesMenu label="Trace" title="Trace a test through the changes in Traces" directory={session.directory} git={session.git} {mainBranch} urlFor={traceShowUrl} class={ACTION_BUTTON_CLASS} />
    {/if}
    <GitMenu name={session.name} git={session.git} {mainBranch} onMerge={actions.merge} onClose={actions.close} class={ACTION_BUTTON_CLASS} />
  </div>
</div>
