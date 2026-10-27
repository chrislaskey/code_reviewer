<!-- The lines of a diff with two line number gutters, signs and syntax colours. -->
<script>
  import { highlightDiffLines } from "./syntax_highlight.js";

  /** @type {{ lines: any[], language: string | null }} */
  let { lines, language } = $props();

  const LINE_STYLES = {
    context: { row: "", sign: " ", signText: "" },
    added: { row: "bg-success/15", sign: "+", signText: "text-success" },
    removed: { row: "bg-error/15", sign: "−", signText: "text-error" },
  };

  let tokens = $derived(highlightDiffLines(lines, language));

  /** True when lines were left out between the two, judged by their line numbers. */
  function hasGapBetween(previous, line) {
    if (!previous) return false;
    const jumped = (before, after) => before !== null && after !== null && after > before + 1;
    return jumped(previous.old_number, line.old_number) || jumped(previous.new_number, line.new_number);
  }
</script>

<div class="overflow-x-auto pb-2 font-mono text-[11px] leading-5">
  <div class="w-max min-w-full">
    {#each lines as line, index (index)}
      {@const style = LINE_STYLES[line.kind]}
      {#if hasGapBetween(lines[index - 1], line)}
        <div class="select-none bg-base-content/5 pl-20 text-base-content/35">⋯</div>
      {/if}
      <div class={["flex", style.row]}>
        <span class="w-9 shrink-0 select-none pr-2 text-right text-base-content/35">{line.old_number ?? ""}</span>
        <span class="w-9 shrink-0 select-none pr-2 text-right text-base-content/35">{line.new_number ?? ""}</span>
        <span class={["w-4 shrink-0 select-none text-center font-bold", style.signText]}>{style.sign}</span>
        <span class="code-syntax whitespace-pre pr-4">
          {#if tokens[index]}
            {#each tokens[index] as token, tokenIndex (tokenIndex)}
              {#if token.className}<span class={token.className}>{token.text}</span>{:else}{token.text}{/if}
            {/each}
          {:else}
            {line.text}
          {/if}
        </span>
      </div>
    {/each}
  </div>
</div>
