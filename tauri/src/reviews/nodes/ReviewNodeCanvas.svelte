<!-- The canvas of group rows. Nodes read what to highlight and what a click
     does from the context set here, so they stay presentational. -->
<script>
  import { onMount, setContext } from "svelte";
  import { Background, BackgroundVariant, Controls, SvelteFlow, useSvelteFlow } from "@xyflow/svelte";
  import "@xyflow/svelte/dist/style.css";
  import GroupCardNode from "./GroupCardNode.svelte";
  import LinkChipNode from "./LinkChipNode.svelte";
  import { buildGroupGraph, groupNodeId } from "./layout.js";

  /**
   * @type {{
   *   rows: any[],
   *   onOpenFile: (groupId: string, fileId: string) => void,
   *   onShowGroup: (groupId: string) => void,
   * }}
   */
  let { rows, onOpenFile, onShowGroup } = $props();

  const nodeTypes = { group_card: GroupCardNode, link_chip: LinkChipNode };
  const { fitView } = useSvelteFlow();

  let nodes = $state.raw([]);
  let edges = $state.raw([]);
  $effect(() => {
    const graph = buildGroupGraph(rows);
    nodes = graph.nodes;
    edges = graph.edges;
  });

  // A group asked for before the canvas has measured its nodes is shown once it has.
  let initialized = false;
  let pendingGroupId = null;

  /** Moves the canvas to the group's row. */
  export function showGroup(groupId) {
    if (!initialized) {
      pendingGroupId = groupId;
      return;
    }
    fitView({ nodes: [{ id: groupNodeId(groupId) }], padding: 0.4, maxZoom: 1, duration: 500 });
  }

  function showPendingGroup() {
    initialized = true;
    if (pendingGroupId !== null) showGroup(pendingGroupId);
    pendingGroupId = null;
  }

  /** The content line the pointer is over: its chips stay, the others fade. */
  const focus = $state({ key: null });

  setContext("review-nodes", {
    focus,
    hoverLine: (key) => {
      focus.key = key;
    },
    openFile: (groupId, fileId) => onOpenFile(groupId, fileId),
    showGroup: (groupId) => onShowGroup(groupId),
  });

  // The canvas follows the page's theme, which the theme toggle sets on <html>.
  /** @returns {"light" | "dark"} */
  function currentColorMode() {
    return document.documentElement.getAttribute("data-theme") === "dark" ? "dark" : "light";
  }

  let colorMode = $state(currentColorMode());
  onMount(() => {
    const observer = new MutationObserver(() => {
      colorMode = currentColorMode();
    });
    observer.observe(document.documentElement, { attributes: true, attributeFilter: ["data-theme"] });
    return () => observer.disconnect();
  });
</script>

<SvelteFlow
  bind:nodes
  bind:edges
  {nodeTypes}
  {colorMode}
  initialViewport={{ x: 24, y: 24, zoom: 0.85 }}
  minZoom={0.2}
  maxZoom={1.5}
  nodesDraggable={false}
  nodesConnectable={false}
  elementsSelectable={false}
  panOnScroll={true}
  zoomOnScroll={false}
  oninit={showPendingGroup}
>
  <Background variant={BackgroundVariant.Dots} gap={18} size={1} />
  <Controls showLock={false} />
</SvelteFlow>
