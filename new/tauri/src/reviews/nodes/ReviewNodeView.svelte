<!-- The Node view: every group row on a canvas. The provider lets the canvas
     move the viewport, see ReviewNodeCanvas. -->
<script>
  import { SvelteFlowProvider } from "@xyflow/svelte";
  import ReviewNodeCanvas from "./ReviewNodeCanvas.svelte";

  /** @type {{ rows: any[], onOpenFile: (groupId: string, fileId: string) => void }} */
  let { rows, onOpenFile } = $props();
</script>

<SvelteFlowProvider>
  <ReviewNodeCanvas {rows} {onOpenFile} />
</SvelteFlowProvider>
