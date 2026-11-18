<!-- Join or Leave, for the community, by whether the person is a member;
     `onchange` runs once the membership changed. -->
<script>
  import { membershipsJoin, membershipsLeave } from "../lib/core/index.js";
  import { currentCommunityChanged } from "../lib/current_community.js";
  import { Button } from "../lib/components/index.js";

  /**
   * @typedef {Object} Props
   * @property {string} communityId
   * @property {object | null} membership the person's, when they are in it
   * @property {() => void} onchange
   * @property {(error: string) => void} onerror
   */

  /** @type {Props} */
  let { communityId, membership, onchange, onerror } = $props();

  let busy = $state(false);

  async function toggle() {
    busy = true;
    try {
      await (membership ? membershipsLeave(communityId) : membershipsJoin(communityId));
      currentCommunityChanged();
      onchange();
    } catch (problem) {
      onerror(String(problem));
    } finally {
      busy = false;
    }
  }
</script>

<Button size="sm" variant={membership ? "ghost" : "default"} onclick={toggle} disabled={busy}>
  {membership ? "Leave" : "Join"}
</Button>
