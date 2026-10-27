<script>
  import ClaudeCodePill from "./ClaudeCodePill.svelte";
  import GitStatus from "./GitStatus.svelte";
  import Pill from "./Pill.svelte";
  import ProcessList from "./ProcessList.svelte";
  import ShortPath from "./ShortPath.svelte";

  let { tmuxWindow, pane, homeDirectory } = $props();

  let program = $derived(pane.foreground_program ?? pane.current_command);
</script>

<div class="pane">
  <span
    class="pane-location"
    class:active={tmuxWindow.active && pane.active}
    title="Pane {pane.id}{pane.active ? ", the window's current pane" : ''}"
  >
    {tmuxWindow.index}.{pane.index}
  </span>

  {#if pane.state === "at_prompt"}
    <Pill text="{program} · idle" kind="pill-good" title="A shell is waiting for a command" />
  {:else if pane.state === "running"}
    <Pill text={program} kind="pill-busy" title="{program} is running in the foreground" />
  {:else if pane.state === "exited"}
    <Pill text="exited" title="The pane's program exited" />
  {:else}
    <Pill text="unknown" title="No foreground process found" />
  {/if}

  {#if pane.claude_code}
    <ClaudeCodePill session={pane.claude_code} />
  {:else}
    <span></span>
  {/if}

  <ShortPath path={pane.current_path} {homeDirectory} />

  {#if pane.git}
    <GitStatus git={pane.git} />
  {:else}
    <span></span>
  {/if}

  <ProcessList processes={pane.processes} />
</div>
