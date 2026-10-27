<!-- The Node view: every group row on a canvas. The provider lets the canvas
     move the viewport, see ReviewNodeCanvas. -->
<script>
  import { SvelteFlowProvider } from "@xyflow/svelte";
  import ReviewNodeCanvas from "./ReviewNodeCanvas.svelte";

  /**
   * @type {{
   *   rows: any[],
   *   onOpenFile: (groupId: string, fileId: string) => void,
   *   onShowGroup: (groupId: string) => void,
   * }}
   */
  let { rows, onOpenFile, onShowGroup } = $props();

  let canvas = $state();

  /** Moves the canvas to the group's row. */
  export function showGroup(groupId) {
    canvas?.showGroup(groupId);
  }
</script>

<SvelteFlowProvider>
  <ReviewNodeCanvas bind:this={canvas} {rows} {onOpenFile} {onShowGroup} />
</SvelteFlowProvider>
