<!-- A tmux session's page: the status bar of the sessions page, with the same
     actions, and the chat history of the agent running in it, read from the
     transcript the agent writes, Claude Code's or Cursor's. The session is
     named in the path, `/sessions/<name>`. Refreshes like the sessions page. -->
<script>
  import { onMount } from "svelte";
  import AppNav from "../lib/components/AppNav.svelte";
  import Button from "../lib/components/Button.svelte";
  import Header from "../lib/components/Header.svelte";
  import InfoPopover from "../lib/components/InfoPopover.svelte";
  import StillWaves from "../lib/components/StillWaves.svelte";
  import TopBackgroundPattern from "../lib/components/TopBackgroundPattern.svelte";
  import {
    getAgentChatHistory,
    getSession,
    getSessionSettings,
    savePromptImageForSession,
    sendPromptToSession,
  } from "../lib/backend.js";
  import { log } from "../lib/log.svelte.js";
  import LogList from "../lib/LogList.svelte";
  import { navigateToUrl } from "../lib/router.js";
  import { sessionNameFromPath, sessionsIndexUrl } from "../lib/routes.js";
  import ChoiceMenu from "../lib/components/ChoiceMenu.svelte";
  import Icon from "../lib/components/Icon.svelte";
  import MenuChoice from "../lib/components/MenuChoice.svelte";
  import { CHAT_FILTERS, filterChatMessages } from "./chat_filters.js";
  import ChatMessage from "./ChatMessage.svelte";
  import ChatSummary from "./ChatSummary.svelte";
  import GitStatus from "./GitStatus.svelte";
  import PromptForm from "./PromptForm.svelte";
  import { readFileAsBase64 } from "./prompt_images.js";
  import { createSessionActions } from "./session_actions.js";
  import SessionStatusBar from "./SessionStatusBar.svelte";

  const REFRESH_INTERVAL_MS = 5000;
  const REFRESH_RIPPLE_STRENGTH = 20;
  // Toggles for the prompt box's layout, each on or off by itself.
  const CHAT_LAYOUT_TOGGLES = [
    { value: "pinned", icon: "lucide-pin", title: "Pin the prompt in view while the chat scrolls" },
    { value: "large", icon: "lucide-maximize-2", title: "Make the prompt box taller" },
  ];

  const name = sessionNameFromPath(window.location.pathname);

  let mainBranch = $state("main");
  let session = $state();
  let sessionError = $state();
  let chat = $state();
  let chatError = $state();
  let chatFilter = $state("all");
  // `pinned` keeps the prompt in view while the chat scrolls; `large` makes it taller.
  let chatLayout = $state({ pinned: false, large: false });
  let chatFilterLabel = $derived(CHAT_FILTERS.find((filter) => filter.value === chatFilter)?.label);
  let shownMessages = $derived(chat ? filterChatMessages(chat.messages, chatFilter) : []);
  // Newest first, each with the one shown before it in time, for "16 seconds later".
  let newestFirst = $derived(
    shownMessages.map((message, index) => ({ message, previous: shownMessages[index - 1] })).reverse(),
  );

  let working = $derived(session?.agent?.status === "busy");
  let promptDisabledReason = $derived(
    !session ? "No running session to write to" : !session.agent?.tmux_pane_id ? "No agent running in the session" : "",
  );

  let waves;
  let refreshing = false;
  let lastRefreshError;

  async function refresh() {
    if (refreshing || name === null) return false;
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
    return !refreshError;
  }

  async function refreshFromButton(event) {
    const box = event.currentTarget.getBoundingClientRect();
    waves.ripple(REFRESH_RIPPLE_STRENGTH, box.left + box.width / 2);
    if (await refresh()) log(`${name}: Refreshed`);
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
    return true;
  }

  async function attachImage(file) {
    try {
      const path = await savePromptImageForSession(name, file.type, await readFileAsBase64(file));
      log(`${name}: Saved the image at ${path}`);
      return path;
    } catch (error) {
      log(`Saving the image for ${name} failed: ${error}`);
      return null;
    }
  }

  function toggleChatLayout(layout) {
    chatLayout[layout] = !chatLayout[layout];
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
      .then(() => (interval = setInterval(refresh, REFRESH_INTERVAL_MS)));
    return () => clearInterval(interval);
  });
</script>

<svelte:window onfocus={refresh} />

{#snippet chatControls()}
  <div class="chat-controls">
    {#each CHAT_LAYOUT_TOGGLES as toggle (toggle.value)}
      <button
        type="button"
        class={["btn btn-ghost btn-sm btn-square", chatLayout[toggle.value] && "btn-active text-primary"]}
        title={toggle.title}
        aria-label={toggle.title}
        aria-pressed={chatLayout[toggle.value]}
        onclick={() => toggleChatLayout(toggle.value)}
      >
        <Icon name={toggle.icon} class="size-4" />
      </button>
    {/each}
    <ChoiceMenu title="Which messages the chat shows" class="btn btn-ghost btn-sm" align="start">
      {#snippet buttonContent()}
        <span class="max-sm:hidden">Show</span>
        {chatFilterLabel}
      {/snippet}
      {#snippet children({ close })}
        {#each CHAT_FILTERS as filter (filter.value)}
          <MenuChoice
            label={filter.label}
            explanation={filter.explanation}
            onclick={() => {
              chatFilter = filter.value;
              close();
            }}
          />
        {/each}
      {/snippet}
    </ChoiceMenu>
  </div>
{/snippet}

<StillWaves bind:this={waves} rippleOnLoad />

<TopBackgroundPattern />

<AppNav current="sessions" />

<main class="page with-waves">
  <Header>
    {name ?? "No session"}
    {#snippet subtitle()}
      The session's status and actions, and the agent's chat history. Refreshes every 5 seconds.
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
    <div class="section-header">
      <div class="section-title">
        <h2>Chat</h2>
        {#if chat}
          <span class="chat-count muted">
            {chatFilter === "all" ? "" : `${shownMessages.length} of `}{chat.messages.length} messages
          </span>
        {/if}
        <InfoPopover title="What the chat holds, and how prompts are sent">
          {#if chat}
            <ChatSummary {chat} />
          {:else}
            Read from the transcript the agent writes as it works, once it has one.
          {/if}
          <p class="mt-2">
            A prompt is typed into the agent's tmux pane, then Enter, as if you typed it there. Nothing is sent until
            you press Enter or Send, and Shift+Enter starts a new line. A pasted, dropped or attached image is saved in the
            session's artifacts and its path put in the prompt, for the agent to read.
          </p>
        </InfoPopover>
      </div>
      <div class="flex items-baseline gap-2">
        <!-- Small windows have it in the status bar's Git menu above. -->
        {#if session?.git}<span class="max-sm:hidden"><GitStatus git={session.git} /></span>{/if}
        {#if name !== null}
          <Button class="btn btn-ghost" onclick={refreshFromButton}>
            <Icon name="hero-arrow-path-micro" class="size-4" />
            Refresh
          </Button>
        {/if}
      </div>
    </div>
    {#if name !== null}
      <div class={["chat-prompt", chatLayout]}>
        <PromptForm
          draftKey={name}
          disabled={promptDisabledReason !== ""}
          disabledReason={promptDisabledReason}
          onSend={sendPrompt}
          onAttachImage={attachImage}
          controls={chatControls}
        />
      </div>
    {/if}
    {#if chat}
      <div class="chat-history">
        {#each newestFirst as { message, previous } (message.id)}
          <ChatMessage
            {message}
            previousTimestamp={previous?.timestamp}
            agent={chat.agent}
            hasToolResults={chat.has_tool_results}
          />
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
  </section>

  <LogList />
</main>
