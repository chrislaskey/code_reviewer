<!-- A browser's back and forward buttons, small carets at the window's top
     edge, beside the macOS window controls' gutter. A direction with nowhere
     to go stays in place, dimmed. Mounted once (main.js), so it outlives
     every page. -->
<script>
  import Icon from "./Icon.svelte";
  import {
    goBackInHistory,
    goForwardInHistory,
    navigationHistory,
    trackNavigationHistory,
  } from "../navigation_history.svelte.js";

  $effect(() => trackNavigationHistory());
</script>

<div class="history-buttons">
  <button
    type="button"
    aria-label="Back"
    title="Back"
    disabled={!navigationHistory.canGoBack}
    onclick={goBackInHistory}
  >
    <Icon name="hero-chevron-left-micro" class="size-4" />
  </button>
  <button
    type="button"
    aria-label="Forward"
    title="Forward"
    disabled={!navigationHistory.canGoForward}
    onclick={goForwardInHistory}
  >
    <Icon name="hero-chevron-right-micro" class="size-4" />
  </button>
</div>

<style>
  /* In line with the window controls, and the first caret's shape in line
     with the page's content (a button's icon sits 8px in from its edge);
     above the title bar's drag strip. */
  .history-buttons {
    position: fixed;
    top: 5px;
    left: calc(var(--gutter-width) + var(--page-gutter) - 8px);
    z-index: 30;
    display: flex;
    gap: 2px;
  }

  button {
    display: grid;
    place-items: center;
    width: 22px;
    height: 20px;
    border-radius: 6px;
    color: color-mix(in oklab, var(--color-base-content) 60%, transparent);
    cursor: pointer;
  }

  button:disabled {
    color: color-mix(in oklab, var(--color-base-content) 22%, transparent);
    cursor: default;
  }

  button:not(:disabled):hover {
    background: color-mix(in oklab, var(--color-base-content) 8%, transparent);
    color: var(--color-base-content);
  }

  /* Small windows, phones among them: Apple's 44pt touch targets, the
     hover shade a smaller square inside. Below the status bar on a phone
     (the safe area), and with no gutter, just past the window controls
     (three 12px circles 8px apart, from 16px in); the page's title keeps
     clear of the taller row (styles.css). */
  @media (width < 40rem) {
    .history-buttons {
      top: env(safe-area-inset-top, 0px);
      left: calc(68px + env(safe-area-inset-left, 0px));
      gap: 0;
    }

    button {
      width: 44px;
      height: 44px;
      padding: 6px;
      border-radius: 12px;
      background-clip: content-box;
    }

    button :global(.size-4) {
      width: 20px;
      height: 20px;
    }
  }
</style>
