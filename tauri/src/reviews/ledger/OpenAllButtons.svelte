<!-- The two icon buttons that open or close every ledger and code block. -->
<script>
  import Icon from "../../lib/components/Icon.svelte";

  /** @type {{ oncommand: (open: boolean) => void }} */
  let { oncommand } = $props();

  const BUTTONS = [
    { open: true, icon: "lucide-chevrons-up-down", title: "Open all the files and their code" },
    { open: false, icon: "lucide-chevrons-down-up", title: "Close all the files and their code" },
  ];
</script>

{#each BUTTONS as button (button.open)}
  <button
    type="button"
    class="btn btn-ghost btn-sm btn-square"
    title={button.title}
    aria-label={button.title}
    onclick={() => oncommand(button.open)}
  >
    <Icon name={button.icon} class="size-4" />
  </button>
{/each}
