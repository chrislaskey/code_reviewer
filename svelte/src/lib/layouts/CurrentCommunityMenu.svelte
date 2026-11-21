<!-- The current community in the nav, by name, as a button that opens a
     dialog in the middle of the screen: the person's communities, a
     filter above them, and a link to the communities page. The filter
     takes the focus on a screen with a mouse, not on a touch screen,
     where the keyboard would cover the list. Enter in the filter
     chooses the first match. Choosing one reloads the page in it. -->
<script>
  import { communitiesList, currentCommunityUpdate, onSynced } from "../core/index.js";
  import { currentCommunityLoad, onCurrentCommunityChanged } from "../current_community.js";
  import { communitiesIndexUrl } from "../routes.js";
  import { Button, Dialog, ErrorMessage, Heading, Icon, Input, Link } from "../components/index.js";

  /** @type {{id: string, name: string} | null} */
  let currentCommunity = $state(null);
  /** @type {{id: string, name: string}[]} */
  let mine = $state([]);
  let filter = $state("");
  let error = $state("");
  let busy = $state(false);
  /** @type {HTMLDialogElement | undefined} */
  let dialog = $state();
  /** @type {HTMLInputElement | undefined} */
  let filterInput = $state();

  let shown = $derived(
    mine.filter((community) => community.name.toLowerCase().includes(filter.trim().toLowerCase())),
  );

  async function load() {
    try {
      currentCommunity = await currentCommunityLoad();
    } catch {
      currentCommunity = null;
    }
  }

  async function open() {
    filter = "";
    error = "";
    dialog?.showModal();
    if (!matchMedia("(pointer: coarse)").matches) filterInput?.focus();
    try {
      mine = (await communitiesList()).filter(({ membership }) => membership !== null).map(({ community }) => community);
    } catch (problem) {
      error = String(problem);
    }
  }

  /** @param {string} communityId */
  async function choose(communityId) {
    busy = true;
    try {
      await currentCommunityUpdate(communityId);
      window.location.reload();
    } catch (problem) {
      error = String(problem);
      busy = false;
    }
  }

  /** @param {KeyboardEvent} event */
  function chooseFirstOnEnter(event) {
    if (event.key !== "Enter" || event.isComposing) return;
    event.preventDefault();
    if (shown.length > 0 && !busy) choose(shown[0].id);
  }

  $effect(() => onSynced(load));
  $effect(() => onCurrentCommunityChanged(load));
  load();
</script>

<Button variant="ghost" size="sm" class="gap-1" onclick={open}>
  <Icon name="hero-user-group-mini" class="size-5" />
  {currentCommunity ? currentCommunity.name : "No community"}
  <Icon name="hero-chevron-down-mini" class="size-4" />
</Button>

<Dialog bind:element={dialog} closeOnBackdrop class="max-h-[80vh]">
  <div class="flex items-center justify-between gap-2">
    <Heading>Communities</Heading>
    <Button variant="ghost" size="sm" class="btn-square" aria-label="Close" onclick={() => dialog?.close()}>
      <Icon name="hero-x-mark-mini" class="size-5" />
    </Button>
  </div>
  <Input
    bind:element={filterInput}
    bind:value={filter}
    onkeydown={chooseFirstOnEnter}
    type="search"
    placeholder="Filter communities"
    aria-label="Filter communities"
  />
  <ErrorMessage message={error} />
  <ul class="menu w-full flex-nowrap overflow-y-auto p-0">
    {#each shown as community (community.id)}
      <li>
        <button
          type="button"
          class={community.id === currentCommunity?.id ? "menu-active" : ""}
          onclick={() => choose(community.id)}
          disabled={busy}
        >
          {community.name}
          {#if community.id === currentCommunity?.id}
            <Icon name="hero-check-mini" class="size-4" />
          {/if}
        </button>
      </li>
    {:else}
      <li class="muted px-3 py-2">
        {mine.length === 0 ? "You are not in a community yet." : "No community matches."}
      </li>
    {/each}
  </ul>
  <Link href={communitiesIndexUrl()} onclick={() => dialog?.close()}>Join / Manage Communities</Link>
</Dialog>
