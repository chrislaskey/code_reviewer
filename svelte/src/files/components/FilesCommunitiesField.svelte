<!-- The communities a file is in, for the New and Edit pages: one
     checkbox for each community the person is a member of, bound to
     `communityIds`. A file is in one at least. A server with a single
     community shows no choice, only an error. -->
<script>
  import { t } from "ttag";
  import { communitiesList } from "../../lib/core/index.js";
  import { currentServerCommunitiesSingle } from "../../lib/current_server.svelte.js";
  import { Checkbox, FieldErrors, Text } from "../../lib/components/index.js";

  /**
   * @typedef {Object} Props
   * @property {string[]} communityIds
   * @property {Record<string, string[]>} [errors]
   * @property {boolean} [disabled]
   */

  /** @type {Props} */
  let { communityIds = $bindable([]), errors = {}, disabled = false } = $props();

  /** The communities the person is in, the ones a file can be put in. */
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

  load();
</script>

{#if currentServerCommunitiesSingle()}
  <FieldErrors {errors} fields={["community_ids"]} />
{:else}
  <fieldset class="flex flex-col gap-1">
    <Text as="legend" tone="muted">{t`In`}</Text>
    <div class="flex flex-wrap gap-3">
      {#each mine as community (community.id)}
        <Checkbox
          checked={communityIds.includes(community.id)}
          {disabled}
          onchange={(event) => communityChoose(community.id, event.currentTarget.checked)}
        >
          {community.name}
        </Checkbox>
      {/each}
    </div>
    <FieldErrors {errors} fields={["community_ids"]} />
  </fieldset>
{/if}
