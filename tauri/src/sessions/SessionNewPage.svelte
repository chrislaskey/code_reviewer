<!-- The new session page: creates a git worktree of a repository and a tmux
     session named with the app's prefix in it, with a port of its own in
     PORT and a command typed in, opens it in the terminal app and goes to the
     session's page. -->
<script>
  import { navigateToUrl } from "../lib/router.js";
  import { sessionShowUrl, sessionsIndexUrl } from "../lib/routes.js";
  import { onMount } from "svelte";
  import AppNav from "../lib/components/AppNav.svelte";
  import Button from "../lib/components/Button.svelte";
  import Header from "../lib/components/Header.svelte";
  import InfoPopover from "../lib/components/InfoPopover.svelte";
  import StillWaves from "../lib/components/StillWaves.svelte";
  import TopBackgroundPattern from "../lib/components/TopBackgroundPattern.svelte";
  import { createSession, getDefaultSessionPort, getSessionSettings } from "../lib/backend.js";
  import { log } from "../lib/log.svelte.js";

  let newSession = $state({ name: "", repository: "", command: "claude", port: "" });
  let creating = $state(false);
  let prefix = $state("");

  async function submit(event) {
    event.preventDefault();
    const name = newSession.name.trim();
    creating = true;
    try {
      const sessionName = await createSession({
        name,
        repository: newSession.repository,
        command: newSession.command,
        port: Number(newSession.port),
      });
      navigateToUrl(sessionShowUrl(sessionName));
    } catch (error) {
      log(`Creating ${name} failed: ${error}`);
      creating = false;
    }
  }

  onMount(() => {
    getSessionSettings()
      .then((detected) => {
        prefix = detected.settings.session_name_prefix;
        if (newSession.repository === "") newSession.repository = detected.directory;
      })
      .catch((error) => log(`Detecting the settings failed: ${error}`));
    getDefaultSessionPort()
      .then((port) => {
        if (newSession.port === "") newSession.port = String(port);
      })
      .catch((error) => log(`Finding a free port failed: ${error}`));
  });
</script>

<StillWaves rippleOnLoad />

<TopBackgroundPattern />

<AppNav current="sessions" />

<main class="page with-waves">
  <p><a href={sessionsIndexUrl()}>← Sessions</a></p>
  <Header>
    <span class="inline-flex items-center gap-2">
      New session
      <InfoPopover title="What a new session creates, and what each field does">
        <dl class="field-help">
          <dt>What it does</dt>
          <dd>
            Creates a git worktree of a repository and a tmux session, both named {prefix}-…, running your shell in
            the worktree with a command typed in, and opens it in your terminal app.
          </dd>
          <dt>Name</dt>
          <dd>
            The worktree goes in the repository's <code>.worktrees/{prefix}-&lt;name&gt;</code> directory, on a new
            branch of that name from the repository's current commit, or on the existing branch. The prefix tells the
            sessions list which tmux sessions and worktrees were made here.
          </dd>
          <dt>Repository</dt>
          <dd>The repository to make the worktree of. A leading <code>~</code> is your home directory.</dd>
          <dt>Command</dt>
          <dd>The command typed into the shell. Leave it empty for a shell at a prompt.</dd>
          <dt>Port</dt>
          <dd>
            Set in the session's <code>PORT</code> variable, for the servers started in the worktree to listen on, so
            each session's app runs on a port of its own. The suggested one is the lowest from 4001 up that no
            session started here holds.
          </dd>
        </dl>
      </InfoPopover>
    </span>
  </Header>

  <form class="panel new-session" onsubmit={submit}>
    <label>
      Name
      <span class="prefixed-input">
        <span class="prefix" title="Sessions started here are named with this prefix">{prefix}-</span>
        <input bind:value={newSession.name} required spellcheck="false" placeholder="fix-login" />
      </span>
    </label>
    <label class="directory-field">
      Repository
      <input bind:value={newSession.repository} required spellcheck="false" placeholder="Detecting…" />
    </label>
    <label>
      Command
      <input bind:value={newSession.command} spellcheck="false" />
    </label>
    <label class="port-field">
      Port
      <input bind:value={newSession.port} type="number" min="1024" max="65535" required placeholder="Finding…" />
    </label>
    <Button variant="primary" type="submit" disabled={creating}>{creating ? "Creating…" : "Create and open"}</Button>
  </form>
</main>
