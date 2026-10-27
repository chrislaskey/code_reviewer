<!-- The new session page: creates a git worktree of a repository and a tmux
     session named with the app's prefix in it, running a command, opens it in
     the terminal app and returns to the list. -->
<script>
  import { sessionsIndexUrl } from "../lib/routes.js";
  import { onMount } from "svelte";
  import AppNav from "../lib/components/AppNav.svelte";
  import Button from "../lib/components/Button.svelte";
  import Header from "../lib/components/Header.svelte";
  import { createSession, getSessionSettings } from "../lib/backend.js";
  import { log } from "../lib/log.svelte.js";
  import LogList from "../lib/LogList.svelte";

  let newSession = $state({ name: "", repository: "", command: "claude" });
  let creating = $state(false);
  let prefix = $state("");

  async function submit(event) {
    event.preventDefault();
    const name = newSession.name.trim();
    creating = true;
    try {
      await createSession({ name, repository: newSession.repository, command: newSession.command });
      window.location.href = sessionsIndexUrl();
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
  });
</script>

<AppNav current="sessions" />

<main class="page wide">
  <p><a href={sessionsIndexUrl()}>← Sessions</a></p>
  <Header>
    New session
    {#snippet subtitle()}
      A git worktree of a repository and a tmux session, both named {prefix}-…, running your shell in the worktree
      with a command typed in, opened in your terminal app.
    {/snippet}
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
    <Button variant="primary" type="submit" disabled={creating}>{creating ? "Creating…" : "Create and open"}</Button>
  </form>
  <p class="muted hint">
    The worktree goes in the repository's <code>.worktrees/{prefix}-&lt;name&gt;</code> directory, on a new branch of
    that name from the repository's current commit, or on the existing branch. The prefix tells the sessions list which
    tmux sessions and worktrees were made here. A leading <code>~</code> is your home directory. Leave the command empty
    for a shell at a prompt.
  </p>

  <LogList />
</main>
