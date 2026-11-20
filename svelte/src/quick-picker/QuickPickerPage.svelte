<!-- The quick picker's window in the desktop app, opened by ⌃⌘T
     (../../../tauri/src/quick_picker/): a grid of the signed-in person's
     images under a search box, the files index's grid in a box of its
     own, searched and fetched a page at a time as the files index is.
     The arrows move the selection; Enter or a click copies the image and
     pastes it where the person was typing; Escape closes. With nobody
     signed in, it says so and opens the app. -->
<script>
  import { tick } from "svelte";
  import FileImage from "../lib/FileImage.svelte";
  import TileGrid from "../lib/TileGrid.svelte";
  import {
    currentUserGet,
    filesContentUrl,
    filesSearchWaitMs,
    onQuickPickerOpened,
    quickPickerAccessibilityRequest,
    quickPickerClose,
    quickPickerFilesSearch,
    quickPickerOpenApp,
    quickPickerPaste,
  } from "../lib/core/index.js";
  import { PagedSearch } from "../lib/paged_search.svelte.js";
  import { fileSize } from "../files/file_size.js";

  let signedIn = $state(true);
  let name = $state("");
  let selected = $state(0);
  let columns = $state(1);
  let message = $state("");
  let accessibilityTrusted = $state(true);
  /** @type {HTMLInputElement | undefined} */
  let input = $state();
  /** @type {HTMLButtonElement | undefined} */
  let openAppButton = $state();
  /** @type {HTMLDivElement | undefined} */
  let scroller = $state();
  /** @type {TileGrid | undefined} */
  let grid = $state();

  const search = new PagedSearch((offset, limit) => quickPickerFilesSearch({ name, offset, limit }));
  /** @type {ReturnType<typeof setTimeout> | undefined} */
  let searchTimer;

  let selectedFile = $derived(search.rows[selected]?.file);

  /** @param {{accessibility_trusted: boolean}} [opened] */
  async function open(opened) {
    clearTimeout(searchTimer);
    name = "";
    message = "";
    accessibilityTrusted = opened?.accessibility_trusted ?? true;
    toTop();
    focus();
    let user = null;
    try {
      user = await currentUserGet();
    } catch (problem) {
      message = String(problem);
    }
    signedIn = Boolean(user);
    if (signedIn) await search.startOverFromTop();
    focus();
  }

  async function focus() {
    await tick();
    (signedIn ? input : openAppButton)?.focus();
  }

  function toTop() {
    selected = 0;
    scroller?.scrollTo({ top: 0 });
  }

  function searchAfterPause() {
    clearTimeout(searchTimer);
    searchTimer = setTimeout(() => {
      toTop();
      search.startOverFromTop();
    }, filesSearchWaitMs);
  }

  /** @param {{id: string} | undefined} file */
  async function paste(file) {
    if (!file) return;
    message = "Copying…";
    try {
      await quickPickerPaste(file.id);
      message = "";
    } catch (problem) {
      message = String(problem);
    }
  }

  async function openApp() {
    try {
      await quickPickerOpenApp();
    } catch (problem) {
      message = String(problem);
    }
  }

  async function allowAccessibility() {
    accessibilityTrusted = await quickPickerAccessibilityRequest();
  }

  /** @param {SubmitEvent} event */
  function onSubmit(event) {
    event.preventDefault();
    paste(selectedFile);
  }

  /** How far each arrow moves the selection. */
  const STEPS = { ArrowLeft: () => -1, ArrowRight: () => 1, ArrowUp: () => -columns, ArrowDown: () => columns };

  /** @param {KeyboardEvent} event */
  function onKeydown(event) {
    if (event.key === "Escape") {
      event.preventDefault();
      quickPickerClose();
    } else if (event.key in STEPS && search.total) {
      event.preventDefault();
      const step = STEPS[/** @type {keyof typeof STEPS} */ (event.key)]();
      select(Math.min(search.total - 1, Math.max(0, selected + step)));
    }
  }

  /** @param {number} position */
  function select(position) {
    selected = position;
    grid?.scrollToPosition(position);
  }

  $effect(() => onQuickPickerOpened(open));
  $effect(() => () => clearTimeout(searchTimer));
  open();
</script>

<svelte:window onkeydown={onKeydown} />

{#if signedIn}
  <form class="quick-picker" onsubmit={onSubmit}>
    <input
      bind:this={input}
      bind:value={name}
      oninput={searchAfterPause}
      type="text"
      placeholder="Search images by name…"
      autocomplete="off"
      spellcheck="false"
    />
    <div class="quick-picker-files" bind:this={scroller}>
      {#if search.total && scroller}
        <TileGrid
          bind:this={grid}
          bind:columns
          total={search.total}
          rows={search.rows}
          {scroller}
          onshow={(start, end) => search.showPositions(start, end)}
        >
          {#snippet tile(/** @type {any} */ { file }, /** @type {number} */ position)}
            <button
              type="button"
              tabindex="-1"
              title={file.name}
              aria-label={file.name}
              aria-pressed={position === selected}
              class={[
                "flex size-full items-center justify-center rounded-box bg-base-200 p-1 hover:bg-base-300",
                position === selected && "bg-primary/20 ring-2 ring-primary ring-inset",
              ]}
              onclick={() => paste(file)}
            >
              <FileImage src={filesContentUrl(file.id)} alt="" class="size-full object-contain" missing="" />
            </button>
          {/snippet}
        </TileGrid>
      {:else if search.total === 0}
        <p class="muted empty">{name ? "No images have that in their name" : "No images yet"}</p>
      {/if}
    </div>
    <footer>
      {#if message || search.error}
        <p class="error" title={message || search.error}>{message || search.error}</p>
      {:else if !accessibilityTrusted}
        <p>
          Copies only: <button type="button" class="link" onclick={allowAccessibility}>allow Accessibility access</button> to paste.
        </p>
      {/if}
      <div class="status">
        <p class="selected-file">{selectedFile ? `${selectedFile.name} · ${fileSize(selectedFile.size)}` : ""}</p>
        <p class="keys"><kbd>←</kbd><kbd>↑</kbd><kbd>↓</kbd><kbd>→</kbd> select · <kbd>↵</kbd> paste · <kbd>esc</kbd> close</p>
      </div>
    </footer>
  </form>
{:else}
  <div class="quick-picker">
    <div class="quick-picker-signed-out">
      <p>Sign in to Starter Pack to pick from your images.</p>
      <button type="button" class="btn btn-primary btn-sm" bind:this={openAppButton} onclick={openApp}>Open Starter Pack</button>
      {#if message}
        <p class="error">{message}</p>
      {/if}
    </div>
    <footer>
      <p class="keys"><kbd>↵</kbd> open · <kbd>esc</kbd> close</p>
    </footer>
  </div>
{/if}
