<!-- The sessions page: the tmux sessions this app started, named with the
     app's prefix, with the Claude Code status in each, then its git worktrees
     with no session running, to continue. The Rust side detects the shell,
     terminal app and tmux once and does the work (see
     src-tauri/src/sessions.rs). -->
<script>
  import { sessionNewUrl } from "../lib/routes.js";
  import { onMount } from "svelte";
  import AppNav from "../lib/components/AppNav.svelte";
  import Button from "../lib/components/Button.svelte";
  import Header from "../lib/components/Header.svelte";
  import Icon from "../lib/components/Icon.svelte";
  import InfoPopover from "../lib/components/InfoPopover.svelte";
  import StillWaves from "../lib/components/StillWaves.svelte";
  import {
    closeSession,
    continueSession,
    getSessionSettings,
    listSessions,
    mergeSession,
    openSessionInBrowser,
    showSessionInTerminalApp,
  } from "../lib/backend.js";
  import { log } from "../lib/log.svelte.js";
  import LogList from "../lib/LogList.svelte";
  import GitWorktree from "./GitWorktree.svelte";
  import { countOf } from "./formatting.js";
  import { listSessionChanges } from "./session_changes.js";
  import TmuxSession from "./TmuxSession.svelte";

  const REFRESH_INTERVAL_MS = 5000;

  const RIPPLE_STRENGTHS = { added: 40, removed: 25, statusChanged: 30, refreshed: 20, action: 25 };

  const OUTCOMES = {
    focused: "Brought its window to the front",
    opened: "Opened a new window",
  };

  let detected = $state();
  let detectError = $state();
  let sessionList = $state();
  let waves;

  let prefix = $derived(detected?.settings.session_name_prefix ?? "");
  let watchedDirectories = $derived(detected?.settings.watched_directories.join(", ") ?? "");
  let mainBranch = $derived(detected?.settings.main_branch ?? "main");
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
      ["Main branch", settings.main_branch],
    ];
  });

  let refreshing = false;
  let lastRefreshError;

  // Returns whether it listed the sessions.
  async function refresh() {
    if (refreshing) return false;
    refreshing = true;
    try {
      const nextSessionList = await listSessions();
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
    for (const { name, from, to } of changes.statusChanged) log(`${name}: Claude Code ${from} → ${to}`);
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

  async function showSession(name) {
    try {
      const outcome = await showSessionInTerminalApp(name);
      log(`${name}: ${OUTCOMES[outcome] ?? outcome}`);
    } catch (error) {
      log(`Opening ${name} failed: ${error}`);
    }
    await refresh();
  }

  async function openInBrowser(name) {
    try {
      log(`${name}: Opened ${await openSessionInBrowser(name)}`);
    } catch (error) {
      log(`Opening ${name} in the browser failed: ${error}`);
    }
  }

  async function continueInWorktree(name) {
    try {
      const outcome = await continueSession(name);
      log(`${name}: ${OUTCOMES[outcome] ?? outcome}`);
    } catch (error) {
      log(`Continuing ${name} failed: ${error}`);
    }
    await refresh();
  }

  async function merge(name) {
    try {
      log(await mergeSession(name));
    } catch (error) {
      log(`Merging ${name} failed: ${error}`);
    }
    await refresh();
  }

  async function close(name) {
    try {
      log(await closeSession(name));
    } catch (error) {
      log(`Closing ${name} failed: ${error}`);
    }
    await refresh();
  }

  // The clipboard API can be unavailable in the webview, so fall back to
  // selecting the text for the user to copy. Returns whether it copied.
  async function copyText(text, element) {
    try {
      await navigator.clipboard.writeText(text);
      log(`Copied: ${text}`);
      return true;
    } catch {
      window.getSelection().selectAllChildren(element);
      log("Could not copy, the command is selected instead");
      return false;
    }
  }

  onMount(() => {
    let interval;
    getSessionSettings()
      .then((settings) => (detected = settings))
      .catch((error) => (detectError = String(error)))
      .then(refresh)
      .then(() => (interval = setInterval(refresh, REFRESH_INTERVAL_MS)));
    return () => clearInterval(interval);
  });
</script>

<svelte:window onfocus={refresh} />

<StillWaves bind:this={waves} />

<AppNav current="sessions" />

<main class="page wide with-waves">
  <Header>
    Sessions
    {#snippet subtitle()}
      The tmux sessions started here, with the Claude Code status in each, then the worktrees with no session running.
      Refreshes every 5 seconds.
    {/snippet}
    {#snippet actions()}
      <Button variant="primary" href={sessionNewUrl()}>New session</Button>
    {/snippet}
  </Header>

  <section>
    <div class="section-header">
      <div class="section-title">
        <h2>Current sessions</h2>
        <InfoPopover title="Which sessions are listed, and what the actions do">
          Only tmux sessions named {prefix}-… are listed, and the worktrees named {prefix}-… of the repositories in
          {watchedDirectories} that have no session, as after a restart. Open new tmux session starts a session in the worktree and types
          <code>claude --continue</code>. The Git menu finishes one: Merge into {mainBranch} fast-forwards {mainBranch} with the
          branch, removes the worktree, deletes the branch and closes the session; Turn into branch removes the worktree and
          keeps the branch. Both need a clean worktree. The
          Claude Code status is read from <code>~/.claude/sessions</code>, which Claude Code doesn't document, so it may
          change.
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
            creates, the directories whose repositories are looked in for worktrees, and the branch the Git menu merges into.
            New sessions run the shell, and Open uses the terminal app.
          </p>
        </InfoPopover>
      </div>
      <Button class="btn btn-ghost btn-sm" onclick={refreshFromButton}>
        <Icon name="hero-arrow-path-micro" class="size-4" />
        Refresh
      </Button>
    </div>
    <div class="session-list" {@attach rippleOnActions}>
      {#if empty}
        <div class="panel empty muted">No sessions or worktrees. New session starts one, named {prefix}-…</div>
      {/if}
      {#each sessionList?.sessions ?? [] as session (session.name)}
        <TmuxSession
          {session}
          {mainBranch}
          onOpen={showSession}
          onOpenInBrowser={openInBrowser}
          onCopy={copyText}
          onMerge={merge}
          onClose={close}
        />
      {/each}
      {#each sessionList?.worktrees ?? [] as worktree (worktree.path)}
        <GitWorktree
          {worktree}
          {mainBranch}
          onContinue={continueInWorktree}
          onMerge={merge}
          onClose={close}
        />
      {/each}
    </div>
  </section>

  <LogList />
</main>
