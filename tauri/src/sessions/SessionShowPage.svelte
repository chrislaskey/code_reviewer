<!-- A tmux session's page: the status bar of the sessions page, with the same
     actions, and the chat history of the agent running in it, read from the
     transcript the agent writes (only Claude Code's so far). The session is
     named in the path, `/sessions/<name>`. Refreshes like the sessions page. -->
<script>
  import { onMount, tick } from "svelte";
  import AppNav from "../lib/components/AppNav.svelte";
  import Button from "../lib/components/Button.svelte";
  import Header from "../lib/components/Header.svelte";
  import StillWaves from "../lib/components/StillWaves.svelte";
  import TopBackgroundPattern from "../lib/components/TopBackgroundPattern.svelte";
  import { getAgentChatHistory, getSession, getSessionSettings, sendPromptToSession } from "../lib/backend.js";
  import { log } from "../lib/log.svelte.js";
  import LogList from "../lib/LogList.svelte";
  import { navigateToUrl } from "../lib/router.js";
  import { sessionNameFromPath, sessionsIndexUrl } from "../lib/routes.js";
  import ChatMessage from "./ChatMessage.svelte";
  import PromptForm from "./PromptForm.svelte";
  import { createSessionActions } from "./session_actions.js";
  import SessionStatusBar from "./SessionStatusBar.svelte";

  const REFRESH_INTERVAL_MS = 5000;

  const name = sessionNameFromPath(window.location.pathname);

  let mainBranch = $state("main");
  let session = $state();
  let sessionError = $state();
  let chat = $state();
  let chatError = $state();
  let chatHistoryElement = $state();

  let working = $derived(session?.agent?.status === "busy");
  let promptDisabledReason = $derived(
    !session ? "No running session to write to" : !session.agent?.tmux_pane_id ? "No agent running in the session" : "",
  );

  let refreshing = false;
  let lastRefreshError;

  async function refresh() {
    if (refreshing || name === null) return;
    refreshing = true;
    try {
      session = await getSession(name);
      sessionError = undefined;
    } catch (error) {
      sessionError = String(error);
    }
    try {
      chat = session?.agent ? await getAgentChatHistory(name) : undefined;
      chatError = undefined;
    } catch (error) {
      chatError = String(error);
    }
    // Refreshes repeat, so only log an error once until it changes.
    const refreshError = sessionError ?? chatError;
    if (refreshError && refreshError !== lastRefreshError) log(`Refreshing ${name} failed: ${refreshError}`);
    lastRefreshError = refreshError;
    refreshing = false;
  }

  function scrollToLatestMessage() {
    chatHistoryElement?.lastElementChild?.scrollIntoView({ block: "end" });
  }

  async function sendPrompt(prompt) {
    try {
      await sendPromptToSession(name, prompt);
      log(`${name}: Sent the prompt`);
    } catch (error) {
      log(`Sending the prompt to ${name} failed: ${error}`);
      return false;
    }
    // The agent writes the prompt to its transcript as it reads it.
    await refresh();
    await tick();
    scrollToLatestMessage();
    return true;
  }

  const actions = createSessionActions({
    refresh,
    onFinished: () => navigateToUrl(sessionsIndexUrl()),
  });

  onMount(() => {
    let interval;
    getSessionSettings()
      .then((detected) => (mainBranch = detected.settings.main_branch))
      .catch((error) => log(`Detecting the settings failed: ${error}`))
      .then(refresh)
      .then(async () => {
        await tick();
        scrollToLatestMessage();
        interval = setInterval(refresh, REFRESH_INTERVAL_MS);
      });
    return () => clearInterval(interval);
  });
</script>

<svelte:window onfocus={refresh} />

<StillWaves rippleOnLoad />

<TopBackgroundPattern />

<AppNav current="sessions" />

<main class="page with-waves">
  <Header>
    {name ?? "No session"}
    {#snippet subtitle()}
      The session's status and actions, and the agent's chat history. Refreshes every 5 seconds.
    {/snippet}
    {#snippet actions()}
      <Button class="btn btn-ghost" href={sessionsIndexUrl()}>All sessions</Button>
    {/snippet}
  </Header>

  <section>
    {#if name === null}
      <div class="panel empty muted">The address names no session.</div>
    {:else if session}
      <div class="panel" class:working>
        <SessionStatusBar {session} {mainBranch} {actions} showDetailsLink={false} />
      </div>
      {#if sessionError}
        <p class="hint muted">{sessionError}. Showing it as it was last seen.</p>
      {/if}
    {:else if sessionError}
      <div class="panel empty muted">{sessionError}</div>
    {:else}
      <div class="panel empty muted">Loading…</div>
    {/if}
  </section>

  <section class="mt-8">
    <div class="section-title">
      <h2>Chat history</h2>
    </div>
    {#if chat}
      <p class="hint muted mb-3">
        {chat.messages.length} messages, read from <code class="chat-path">{chat.path}</code>
      </p>
      <div class="chat-history" bind:this={chatHistoryElement}>
        {#each chat.messages as message, index (message.id)}
          <ChatMessage {message} previousTimestamp={chat.messages[index - 1]?.timestamp} />
        {/each}
      </div>
    {:else if chatError}
      <div class="panel empty muted">{chatError}</div>
    {:else if !session && sessionError}
      <div class="panel empty muted">No chat history without a running session.</div>
    {:else if session && !session.agent}
      <div class="panel empty muted">No agent is running in the session.</div>
    {:else}
      <div class="panel empty muted">Loading…</div>
    {/if}
    {#if name !== null}
      <PromptForm {working} disabled={promptDisabledReason !== ""} disabledReason={promptDisabledReason} onSend={sendPrompt} />
    {/if}
  </section>

  <LogList />
</main>
