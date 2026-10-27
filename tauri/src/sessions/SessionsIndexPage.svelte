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
  import {
    closeSession,
    continueSession,
    getSessionSettings,
    listSessions,
    mergeSession,
    showSessionInTerminalApp,
  } from "../lib/backend.js";
  import { log } from "../lib/log.svelte.js";
  import LogList from "../lib/LogList.svelte";
  import GitWorktree from "./GitWorktree.svelte";
  import TmuxSession from "./TmuxSession.svelte";

  const REFRESH_INTERVAL_MS = 5000;

  const OUTCOMES = {
    focused: "Brought its window to the front",
    opened: "Opened a new window",
  };

  let detected = $state();
  let detectError = $state();
  let sessionList = $state();

  let prefix = $derived(detected?.settings.session_name_prefix ?? "");
  let watchedDirectories = $derived(detected?.settings.watched_directories.join(", ") ?? "");
  let mainBranch = $derived(detected?.settings.main_branch ?? "main");
  let empty = $derived(sessionList?.sessions.length === 0 && sessionList?.worktrees.length === 0);

  let settingsSummary = $derived.by(() => {
    if (detectError) return `Detecting failed: ${detectError}`;
    if (!detected) return "Detecting…";
    const { settings, terminal_apps } = detected;
    const terminalApp = terminal_apps.find((app) => app.id === settings.terminal_app)?.name ?? settings.terminal_app;
    return [
      settings.shell,
      terminalApp,
      settings.tmux_path,
      `${settings.session_name_prefix}-…`,
      settings.watched_directories.join(", "),
      settings.main_branch,
    ].join(" · ");
  });

  let refreshing = false;
  let lastRefreshError;

  async function refresh() {
    if (refreshing) return;
    refreshing = true;
    try {
      sessionList = await listSessions();
      lastRefreshError = undefined;
    } catch (error) {
      // Refreshes repeat, so only log an error once until it changes.
      if (String(error) !== lastRefreshError) log(`Listing the sessions failed: ${error}`);
      lastRefreshError = String(error);
    } finally {
      refreshing = false;
    }
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
  // selecting the text for the user to copy.
  async function copyText(text, element) {
    try {
      await navigator.clipboard.writeText(text);
      log(`Copied: ${text}`);
    } catch {
      window.getSelection().selectAllChildren(element);
      log("Could not copy, the command is selected instead");
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

<AppNav current="sessions" />

<main class="page wide">
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
      <h2>Sessions named {prefix}-…</h2>
      <Button class="btn btn-primary btn-soft btn-sm" onclick={refresh}>Refresh</Button>
    </div>
    <div class="session-list">
      {#if empty}
        <div class="panel empty muted">No sessions or worktrees. New session starts one, named {prefix}-…</div>
      {/if}
      {#each sessionList?.sessions ?? [] as session (session.name)}
        <TmuxSession
          {session}
          homeDirectory={detected?.home_directory}
          {mainBranch}
          onOpen={showSession}
          onCopy={copyText}
          onMerge={merge}
          onClose={close}
        />
      {/each}
      {#each sessionList?.worktrees ?? [] as worktree (worktree.path)}
        <GitWorktree
          {worktree}
          homeDirectory={detected?.home_directory}
          {mainBranch}
          onContinue={continueInWorktree}
          onMerge={merge}
          onClose={close}
        />
      {/each}
    </div>
    <p class="muted hint">
      Only tmux sessions named {prefix}-… are listed, and the worktrees named {prefix}-… of the repositories in
      {watchedDirectories} that have no session, as after a restart. Continue starts a session in the worktree and types
      <code>claude --continue</code>. Complete finishes one: Merge into {mainBranch} fast-forwards {mainBranch} with the
      branch, removes the worktree, deletes the branch and closes the session; Turn into branch removes the worktree and
      keeps the branch. Both need a clean worktree. The
      Claude Code status is read from <code>~/.claude/sessions</code>, which Claude Code doesn't document, so it may
      change.
    </p>
  </section>

  <details class="panel settings">
    <summary>
      <span>Detected</span>
      <span class="muted">{settingsSummary}</span>
    </summary>
    <p class="muted settings-note">
      The shell, terminal app and tmux the app found on this Mac, the prefix of the sessions and worktrees it creates,
      the directories whose repositories are looked in for worktrees, and the branch Complete merges into. New sessions run
      the shell, and Open uses the terminal app.
    </p>
  </details>

  <LogList />
</main>
