<!-- The sessions page: the tmux sessions this app started, named with the
     app's prefix, with the agent status in each, each opening its own page,
     then its git worktrees with no session running, to start one in. The
     list comes from the backend's cache (server/src/session_summaries.rs),
     so it shows at once and is at most a refresh old. -->
<script>
  import { sessionNewUrl, sessionShowUrl } from "../lib/routes.js";
  import { onMount } from "svelte";
  import AppNav from "../lib/components/AppNav.svelte";
  import Button from "../lib/components/Button.svelte";
  import Header from "../lib/components/Header.svelte";
  import Icon from "../lib/components/Icon.svelte";
  import InfoPopover from "../lib/components/InfoPopover.svelte";
  import StillWaves from "../lib/components/StillWaves.svelte";
  import TopBackgroundPattern from "../lib/components/TopBackgroundPattern.svelte";
  import { continueSession, getSessionSettings, listSessionSummaries } from "../lib/backend.js";
  import { log } from "../lib/log.svelte.js";
  import LogList from "../lib/LogList.svelte";
  import { repeatWhileVisible } from "../lib/repeat_while_visible.js";
  import { navigateToUrl } from "../lib/router.js";
  import { countOf } from "./formatting.js";
  import { SHOW_OUTCOMES } from "./session_actions.js";
  import { listSessionChanges } from "./session_changes.js";
  import SessionSummary from "./SessionSummary.svelte";
  import WorktreeWithNoSession from "./WorktreeWithNoSession.svelte";

  const REFRESH_INTERVAL_MS = 2000;

  const RIPPLE_STRENGTHS = { added: 40, removed: 25, statusChanged: 30, refreshed: 20, action: 25 };

  let detected = $state();
  let detectError = $state();
  let sessionList = $state();
  let waves;

  let prefix = $derived(detected?.settings.session_name_prefix ?? "");
  let watchedDirectories = $derived(detected?.settings.watched_directories.join(", ") ?? "");
  let empty = $derived(sessionList?.sessions.length === 0 && sessionList?.worktrees.length === 0);

  let detectedSettings = $derived.by(() => {
    if (!detected) return [];
    const { settings, terminal_apps } = detected;
    const terminalApp = terminal_apps.find((app) => app.id === settings.terminal_app)?.name ?? settings.terminal_app;
    return [
      ["Shell", settings.shell],
      ["Terminal app", terminalApp],
      ["tmux", settings.tmux_path],
      ["Session prefix", `${settings.session_name_prefix}-…`],
      ["Watched directories", settings.watched_directories.join(", ")],
    ];
  });

  let refreshing = false;
  let lastRefreshError;

  // Returns whether it listed the sessions.
  async function refresh() {
    if (refreshing) return false;
    refreshing = true;
    try {
      const nextSessionList = await listSessionSummaries();
      if (sessionList) showSessionChanges(listSessionChanges(sessionList, nextSessionList));
      sessionList = nextSessionList;
      lastRefreshError = undefined;
      return true;
    } catch (error) {
      // Refreshes repeat, so only log an error once until it changes.
      if (String(error) !== lastRefreshError) log(`Listing the sessions failed: ${error}`);
      lastRefreshError = String(error);
      return false;
    } finally {
      refreshing = false;
    }
  }

  function showSessionChanges(changes) {
    for (const { name, from, to } of changes.statusChanged) log(`${name}: agent ${from} → ${to}`);
    for (const [change, entries] of Object.entries(changes)) {
      for (let i = 0; i < entries.length; i++) waves.ripple(RIPPLE_STRENGTHS[change]);
    }
  }

  function rippleUnder(element, strength) {
    const box = element.getBoundingClientRect();
    waves.ripple(strength, box.left + box.width / 2);
  }

  async function refreshFromButton(event) {
    rippleUnder(event.currentTarget, RIPPLE_STRENGTHS.refreshed);
    if (await refresh()) {
      const sessions = countOf(sessionList.sessions.length, "session");
      const worktrees = countOf(sessionList.worktrees.length, "worktree");
      log(`Refreshed: ${sessions} and ${worktrees} with no session`);
    }
  }

  // Only choices: a menu's own summary isn't a button or link.
  function rippleOnActions(element) {
    const rippleOnAction = (event) => {
      const action = event.target.closest("button, a");
      if (action) rippleUnder(action, RIPPLE_STRENGTHS.action);
    };
    element.addEventListener("click", rippleOnAction);
    return () => element.removeEventListener("click", rippleOnAction);
  }

  // Opens the new session's page once it's running.
  async function continueInWorktree(name) {
    try {
      const outcome = await continueSession(name);
      log(`${name}: ${SHOW_OUTCOMES[outcome] ?? outcome}`);
    } catch (error) {
      log(`Continuing ${name} failed: ${error}`);
      await refresh();
      return;
    }
    navigateToUrl(sessionShowUrl(name));
  }

  onMount(() => {
    let stopRefreshing;
    getSessionSettings()
      .then((settings) => (detected = settings))
      .catch((error) => (detectError = String(error)));
    refresh().then(() => (stopRefreshing = repeatWhileVisible(refresh, REFRESH_INTERVAL_MS)));
    return () => stopRefreshing?.();
  });
</script>

<svelte:window onfocus={refresh} />

<StillWaves bind:this={waves} rippleOnLoad />

<TopBackgroundPattern />

<AppNav current="sessions" />

<main class="page with-waves">
  <Header>
    Sessions
    {#snippet subtitle()}
      The tmux sessions started here, with the agent status in each, then the worktrees with no session running.
      Refreshes every 2 seconds.
    {/snippet}
    {#snippet actions()}
      <Button variant="primary" href={sessionNewUrl()}>New<span class="max-sm:hidden">&nbsp;session</span></Button>
    {/snippet}
  </Header>

  <section>
    <div class="section-header">
      <div class="section-title">
        <h2>Current sessions</h2>
        <InfoPopover title="Which sessions are listed, and what the actions do">
          Only tmux sessions named {prefix}-… are listed, and the worktrees named {prefix}-… of the repositories in
          {watchedDirectories} that have no session, as after a restart. A session opens its page, with the agent's chat
          and the Open, Review, Trace and Git menus. Open new tmux session starts a session in the worktree, types the command
          that resumes the agent last chatting there (<code>claude --continue</code>, <code>codex resume</code> or
          <code>cursor-agent --resume</code>) and opens its page. The agent is Claude Code, Cursor's <code>agent</code> or <code>codex</code>. Claude Code's status is read from
          <code>~/.claude/sessions</code>, which it doesn't document, so it may change. <code>agent</code> and
          <code>codex</code> report none, so each is busy when its chat was written to in the last few seconds, else idle.
        </InfoPopover>
        <InfoPopover icon="hero-cog-6-tooth" title="The settings detected on this Mac">
          {#if detectError}
            Detecting failed: {detectError}
          {:else if !detected}
            Detecting…
          {:else}
            <dl class="detected-settings">
              {#each detectedSettings as [name, value] (name)}
                <dt>{name}</dt>
                <dd>{value}</dd>
              {/each}
            </dl>
          {/if}
          <p class="mt-2">
            The shell, terminal app and tmux the app found on this Mac, the prefix of the sessions and worktrees it
            creates, and the directories whose repositories are looked in for worktrees. New sessions run the shell, and Open uses the terminal app.
          </p>
        </InfoPopover>
      </div>
      <Button class="btn btn-ghost" onclick={refreshFromButton}>
        <Icon name="hero-arrow-path-micro" class="size-4" />
        Refresh
      </Button>
    </div>
    <div class="session-list" {@attach rippleOnActions}>
      {#if empty}
        <div class="panel empty muted">No sessions or worktrees. New session starts one, named {prefix}-…</div>
      {/if}
      {#each sessionList?.sessions ?? [] as session (session.name)}
        <SessionSummary {session} />
      {/each}
      {#each sessionList?.worktrees ?? [] as worktree (worktree.path)}
        <WorktreeWithNoSession {worktree} onContinue={continueInWorktree} />
      {/each}
    </div>
  </section>

  <LogList />
</main>
