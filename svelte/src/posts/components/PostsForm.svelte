<!-- The fields of a post, for the New and Edit pages: the body, bound to
     the page's `text`, the communities it is in among the ones the person
     is a member of, bound to `communityIds`, and the error the write came
     back with. A server with a single community shows no choice. -->
<script>
  import { t } from "ttag";
  import { communitiesList } from "../../lib/core/index.js";
  import { currentServerCommunitiesSingle } from "../../lib/current_server.svelte.js";
  import { Button, Checkbox, ErrorMessage, Text, Textarea } from "../../lib/components/index.js";

  /**
   * @typedef {Object} Props
   * @property {string} text
   * @property {string[]} communityIds
   * @property {string} submitText
   * @property {string} [error]
   * @property {boolean} [busy]
   * @property {() => void} onsubmit
   */

  /** @type {Props} */
  let { text = $bindable(""), communityIds = $bindable([]), submitText, error = "", busy = false, onsubmit } = $props();

  /** The communities the person is in, the ones a post can be put in. */
  let mine = $state([]);

  async function load() {
    try {
      mine = (await communitiesList()).filter(({ membership }) => membership !== null).map(({ community }) => community);
    } catch {
      // Shown by the write, if it matters
    }
  }

  /**
   * @param {string} communityId
   * @param {boolean} chosen
   */
  function communityChoose(communityId, chosen) {
    communityIds = chosen ? [...communityIds, communityId] : communityIds.filter((id) => id !== communityId);
  }

  function submit(event) {
    event.preventDefault();
    onsubmit();
  }

  load();
</script>

<form onsubmit={submit} class="flex flex-col gap-3">
  <Textarea bind:value={text} rows="4" aria-label={t`Post`} placeholder={t`What's on your mind?`} maxlength="10000" />
  {#if mine.length > 0 && !currentServerCommunitiesSingle()}
    <fieldset class="flex flex-wrap gap-3">
      <Text as="legend" tone="muted">{t`In`}</Text>
      {#each mine as community (community.id)}
        <Checkbox
          checked={communityIds.includes(community.id)}
          onchange={(event) => communityChoose(community.id, event.currentTarget.checked)}
        >
          {community.name}
        </Checkbox>
      {/each}
    </fieldset>
  {/if}
  <ErrorMessage message={error} />
  <div>
    <Button variant="primary" type="submit" disabled={busy}>{submitText}</Button>
  </div>
</form>
