<script>
  import Button from "../lib/components/Button.svelte";
  import Pill from "./Pill.svelte";
  import TmuxPane from "./TmuxPane.svelte";

  let { session, homeDirectory, onOpen, onCopy } = $props();

  let attached = $derived(session.attached_clients > 0);
  let attachCommand = $derived(`tmux attach -t ${session.name}`);
  let attachCommandElement;

  // tmux names windows after their program by default, which for Claude Code
  // is its version number, so use the readable program name then.
  function windowName(tmuxWindow) {
    const pane = tmuxWindow.panes.find((pane) => pane.active) ?? tmuxWindow.panes[0];
    if (pane && tmuxWindow.name === pane.current_command && pane.foreground_program) {
      return pane.foreground_program;
    }
    return tmuxWindow.name;
  }
</script>

<div class="panel">
  <div class="session-header">
    <span class="name">{session.name}</span>
    <Pill text={attached ? "attached" : "detached"} kind={attached ? "pill-good" : ""} />
    <div class="attach">
      <code bind:this={attachCommandElement} title="Paste into any terminal to attach">{attachCommand}</code>
      <Button class="btn btn-primary btn-soft btn-sm" onclick={() => onCopy(attachCommand, attachCommandElement)}>Copy</Button>
      <Button class="btn btn-primary btn-sm" onclick={() => onOpen(session.name)}>Open</Button>
    </div>
  </div>

  {#each session.windows as tmuxWindow (tmuxWindow.index)}
    <div class="window-label">
      Window {tmuxWindow.index} · {windowName(tmuxWindow)}{tmuxWindow.active ? " · current" : ""}
    </div>
    {#each tmuxWindow.panes as pane (pane.id)}
      <TmuxPane {tmuxWindow} {pane} {homeDirectory} />
    {/each}
  {/each}
</div>
