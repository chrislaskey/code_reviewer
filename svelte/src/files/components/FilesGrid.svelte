<!-- The files index's grid, a TileGrid on the page's scroll: a square
     for each of `total` files showing its image, or an icon when it is
     not one, and a mark when it is not on the server yet. It calls
     `onshow` with the positions it draws, for the page to fetch the
     files there. See docs/design/files-browsing.md. -->
<script>
  import { t } from "ttag";
  import FileImage from "../../lib/FileImage.svelte";
  import { filesContentUrl } from "../../lib/core/index.js";
  import { filesShowUrl } from "../../lib/routes.js";
  import { Icon, TileGrid } from "../../lib/components/index.js";

  /**
   * @typedef {Object} Props
   * @property {number} total
   * @property {any[]} rows the files by position, a hole where one has not come yet
   * @property {(start: number, end: number) => void} onshow
   */

  /** @type {Props} */
  let { total, rows, onshow } = $props();
</script>

<TileGrid {total} {rows} {onshow}>
  {#snippet tile(/** @type {any} */ { file, shown_as, state })}
    <a
      href={filesShowUrl(file.id)}
      title={file.name}
      class="relative flex size-full items-center justify-center rounded-box bg-base-200 p-1 hover:bg-base-300"
    >
      {#if shown_as === "image"}
        <FileImage src={filesContentUrl(file.id)} alt={file.name} class="size-full object-contain" missing="" />
      {:else}
        <Icon name="hero-document-text" class="size-8 opacity-60" />
        <span class="sr-only">{file.name}</span>
      {/if}
      {#if state === "waiting"}
        <Icon name="hero-cloud-arrow-up-mini" class="absolute top-1 right-1 size-4" />
        <span class="sr-only">{t`Waiting to upload`}</span>
      {:else if state === "refused"}
        <Icon name="hero-exclamation-triangle-mini" class="absolute top-1 right-1 size-4 text-warning" />
        <span class="sr-only">{t`Not saved on the server`}</span>
      {/if}
    </a>
  {/snippet}
</TileGrid>
