<!-- The fields of a post, for the New and Edit pages: the body, bound to
     the page's `text`, the communities it is in among the ones the person
     is a member of, bound to `communityIds`, and the error the write came
     back with. -->
<script>
  import { t } from "ttag";
  import { communitiesList } from "../lib/core/index.js";

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

  function submit(event) {
    event.preventDefault();
    onsubmit();
  }

  load();
</script>

<form onsubmit={submit} class="flex flex-col gap-3">
  <textarea bind:value={text} rows="4" class="textarea w-full" aria-label={t`Post`} placeholder={t`What's on your mind?`} maxlength="10000"
  ></textarea>
  {#if mine.length > 0}
    <fieldset class="flex flex-wrap gap-3">
      <legend class="muted">{t`In`}</legend>
      {#each mine as community (community.id)}
        <label class="flex items-center gap-1">
          <input type="checkbox" class="checkbox checkbox-sm" value={community.id} bind:group={communityIds} />
          {community.name}
        </label>
      {/each}
    </fieldset>
  {/if}
  {#if error}
    <p class="error">{error}</p>
  {/if}
  <div>
    <button type="submit" class="btn btn-primary" disabled={busy}>{submitText}</button>
  </div>
</form>
