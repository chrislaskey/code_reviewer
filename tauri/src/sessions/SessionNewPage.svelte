<!-- The new session page: creates a tmux session running a command in a
     directory and opens it in the terminal app, then returns to the list. -->
<script>
  import { onMount } from "svelte";
  import AppNav from "../lib/components/AppNav.svelte";
  import Button from "../lib/components/Button.svelte";
  import Header from "../lib/components/Header.svelte";
  import { createSession, getSessionSettings } from "../lib/backend.js";
  import { log } from "../lib/log.svelte.js";
  import LogList from "../lib/LogList.svelte";

  let newSession = $state({ name: "claude", directory: "", command: "claude" });
  let creating = $state(false);

  async function submit(event) {
    event.preventDefault();
    const name = newSession.name.trim();
    creating = true;
    try {
      await createSession({ name, directory: newSession.directory, command: newSession.command });
      window.location.href = "/sessions/index.html";
    } catch (error) {
      log(`Creating ${name} failed: ${error}`);
      creating = false;
    }
  }

  onMount(() => {
    getSessionSettings()
      .then((detected) => {
        if (newSession.directory === "") newSession.directory = detected.directory;
      })
      .catch((error) => log(`Detecting the settings failed: ${error}`));
  });
</script>

<AppNav current="sessions" />

<main class="page wide">
  <p><a href="/sessions/index.html">← Sessions</a></p>
  <Header>
    New session
    {#snippet subtitle()}
      A tmux session running your shell in a directory, with a command typed into it, opened in your terminal app.
    {/snippet}
  </Header>

  <form class="panel new-session" onsubmit={submit}>
    <label>
      Name
      <input bind:value={newSession.name} required spellcheck="false" />
    </label>
    <label class="directory-field">
      Directory
      <input bind:value={newSession.directory} required spellcheck="false" placeholder="Detecting…" />
    </label>
    <label>
      Command
      <input bind:value={newSession.command} spellcheck="false" />
    </label>
    <Button variant="primary" type="submit" disabled={creating}>{creating ? "Creating…" : "Create and open"}</Button>
  </form>
  <p class="muted hint">
    A leading <code>~</code> in the directory is your home directory. Leave the command empty for a shell at a prompt.
  </p>

  <LogList />
</main>
