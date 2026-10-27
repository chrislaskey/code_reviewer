<!-- The sessions page: the user's tmux sessions with what runs in each pane,
     and their Claude Code sessions. The Rust side detects the shell, terminal
     app and tmux once and does the work (see src-tauri/src/sessions.rs). -->
<script>
  import { onMount } from "svelte";
  import AppNav from "../lib/components/AppNav.svelte";
  import Button from "../lib/components/Button.svelte";
  import Header from "../lib/components/Header.svelte";
  import { getSessionSettings, listSessions, showSessionInTerminalApp } from "../lib/backend.js";
  import { log } from "../lib/log.svelte.js";
  import LogList from "../lib/LogList.svelte";
  import ClaudeCodeSession from "./ClaudeCodeSession.svelte";
  import TmuxSession from "./TmuxSession.svelte";

  const REFRESH_INTERVAL_MS = 5000;

  const OUTCOMES = {
    focused: "Brought its window to the front",
    opened: "Opened a new window",
  };

  let detected = $state();
  let detectError = $state();
  let sessionList = $state();

  let settingsSummary = $derived.by(() => {
    if (detectError) return `Detecting failed: ${detectError}`;
    if (!detected) return "Detecting…";
    const { settings, terminal_apps } = detected;
    const terminalApp = terminal_apps.find((app) => app.id === settings.terminal_app)?.name ?? settings.terminal_app;
    return [settings.shell, terminalApp, settings.tmux_path].join(" · ");
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
      Your tmux sessions and what runs in them. Refreshes every 5 seconds.
    {/snippet}
    {#snippet actions()}
      <Button variant="primary" href="/sessions/new.html">New session</Button>
    {/snippet}
  </Header>

  <section>
    <div class="section-header">
      <h2>tmux</h2>
      <Button class="btn btn-primary btn-soft btn-sm" onclick={refresh}>Refresh</Button>
    </div>
    <div class="session-list">
      {#if sessionList?.sessions.length === 0}
        <div class="panel empty muted">No tmux sessions.</div>
      {/if}
      {#each sessionList?.sessions ?? [] as session (session.name)}
        <TmuxSession {session} homeDirectory={detected?.home_directory} onOpen={showSession} onCopy={copyText} />
      {/each}
    </div>
  </section>

  <section>
    <h2>Claude Code</h2>
    <ul class="panel rows">
      {#each sessionList?.claude_code_sessions ?? [] as session (session.pid)}
        <ClaudeCodeSession {session} homeDirectory={detected?.home_directory} />
      {:else}
        {#if sessionList}
          <li class="empty muted">No Claude Code sessions running.</li>
        {/if}
      {/each}
    </ul>
    <p class="muted hint">
      Read from <code>~/.claude/sessions</code>, which Claude Code doesn't document, so it may change.
    </p>
  </section>

  <details class="panel settings">
    <summary>
      <span>Detected</span>
      <span class="muted">{settingsSummary}</span>
    </summary>
    <p class="muted settings-note">
      The shell, terminal app and tmux the app found on this Mac. New sessions run the shell, and Open uses the terminal app.
    </p>
  </details>

  <LogList />
</main>
