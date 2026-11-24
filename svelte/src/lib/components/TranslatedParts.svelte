<!-- A translated sentence with pieces of the page in it, such as a link:
     `parts` is ttag's `jt`, the text with the snippets where the
     translation puts them, so the sentence stays one message:
     `<TranslatedParts parts={jt`Read the ${termsLink}.`} />`. -->
<script>
  /**
   * @typedef {Object} Props
   * @property {(string | import("svelte").Snippet)[]} parts
   */

  /** @type {Props} */
  let { parts } = $props();
</script>

{#each parts as part, index (index)}{#if typeof part === "string"}{part}{:else}{@render part()}{/if}{/each}
