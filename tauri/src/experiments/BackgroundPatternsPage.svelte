<!-- An experiments page for background patterns at the top of a page, behind
     the nav and fading out before the content. Click the page or use the
     arrow keys to step through the ideas; the bar at the bottom tunes them,
     and All patterns shows every idea at once. The pattern is in the URL
     hash, so a reload keeps it. -->
<script>
  import AppNav from "../lib/components/AppNav.svelte";
  import Button from "../lib/components/Button.svelte";
  import Header from "../lib/components/Header.svelte";
  import SegmentedControl from "../lib/components/SegmentedControl.svelte";
  import BackgroundPattern from "../lib/components/BackgroundPattern.svelte";
  import { COLORS, FADES, PATTERNS } from "../lib/background_patterns.js";

  const DEFAULT_SETTINGS = { size: 24, angle: -12, opacity: 0.22, height: 420, fade: "top", color: "ink" };

  let index = $state(Math.max(0, PATTERNS.findIndex((pattern) => `#${pattern.id}` === window.location.hash)));
  let settings = $state({ ...DEFAULT_SETTINGS });
  let showingAll = $state(false);

  let pattern = $derived(PATTERNS[index]);

  $effect(() => {
    history.replaceState(null, "", `#${pattern.id}`);
  });

  function step(by) {
    index = (index + by + PATTERNS.length) % PATTERNS.length;
  }

  function choose(chosenIndex) {
    index = chosenIndex;
    showingAll = false;
    window.scrollTo({ top: 0 });
  }

  function stepWhenBackgroundClicked(event) {
    if (showingAll) return;
    if (event.target.closest("a, button, input, select, label, summary, details, .pattern-controls")) return;
    step(1);
  }

  function stepOnArrowKeys(event) {
    if (event.target.closest("input, select, textarea")) return;
    if (event.key === "ArrowRight") step(1);
    if (event.key === "ArrowLeft") step(-1);
  }
</script>

<svelte:window onclick={stepWhenBackgroundClicked} onkeydown={stepOnArrowKeys} />

{#if !showingAll}
  <div class="pattern-backdrop" style="height: {settings.height}px">
    <BackgroundPattern {pattern} {settings} />
  </div>
{/if}

<AppNav />

<main class="page pattern-page">
  <Header>
    Background patterns
    {#snippet subtitle()}
      Ideas for a pattern at the top of each page that fades out before the content. Click anywhere on the page, or
      press ← and →, to try the next one.
    {/snippet}
    {#snippet actions()}
      <Button onclick={() => (showingAll = !showingAll)}>{showingAll ? "Back to one pattern" : "All patterns"}</Button>
    {/snippet}
  </Header>

  {#if showingAll}
    <div class="grid grid-cols-3 gap-4">
      {#each PATTERNS as each, eachIndex (each.id)}
        <button
          type="button"
          onclick={() => choose(eachIndex)}
          class={[
            "relative h-48 overflow-hidden rounded-xl border text-left",
            eachIndex === index ? "border-primary" : "border-base-300 hover:border-base-content/40",
          ]}
        >
          <BackgroundPattern pattern={each} {settings} />
          <span class="absolute inset-x-0 bottom-0 p-3">
            <span class="block font-semibold">{each.name}</span>
            <span class="block text-sm text-base-content/60">{each.idea}</span>
          </span>
        </button>
      {/each}
    </div>
  {:else}
    <section class="space-y-3">
      <h2>Sample content</h2>
      {#each ["cr-fix-login", "cr-trace-pricing", "cr-ui-overhaul"] as name (name)}
        <div class="panel flex items-center gap-3 px-5 py-4">
          <span class="text-lg font-semibold">{name}</span>
          <span class="badge badge-ghost">idle</span>
          <span class="ml-auto text-sm text-base-content/60">Open · Review · Trace · Git</span>
        </div>
      {/each}
      <p class="muted">
        The pattern sits behind the nav and the header and should be gone by the time the content starts. The panels
        above are white, so they cover it where they overlap.
      </p>
    </section>
  {/if}
</main>

<div class="pattern-controls">
  <div class="flex items-center gap-3">
    <button type="button" class="btn btn-ghost btn-sm" onclick={() => step(-1)} title="Previous (←)">←</button>
    <div class="min-w-0 flex-1">
      <p class="font-semibold">
        <span class="font-mono text-sm text-base-content/50">{index + 1}/{PATTERNS.length}</span>
        {pattern.name}
      </p>
      <p class="truncate text-sm text-base-content/60">{pattern.idea}</p>
    </div>
    <button type="button" class="btn btn-ghost btn-sm" onclick={() => step(1)} title="Next (→)">→</button>
  </div>
  <div class="mt-2 flex flex-wrap items-center gap-x-5 gap-y-2 text-sm">
    <label class="flex items-center gap-2">
      Size <input type="range" class="range range-xs w-24" min="8" max="64" bind:value={settings.size} />
      <span class="w-10 font-mono text-xs">{settings.size}px</span>
    </label>
    <label class="flex items-center gap-2">
      Angle <input type="range" class="range range-xs w-24" min="-45" max="45" bind:value={settings.angle} />
      <span class="w-10 font-mono text-xs">{settings.angle}°</span>
    </label>
    <label class="flex items-center gap-2">
      Strength <input type="range" class="range range-xs w-24" min="0.04" max="0.8" step="0.02" bind:value={settings.opacity} />
      <span class="w-10 font-mono text-xs">{Math.round(settings.opacity * 100)}%</span>
    </label>
    <label class="flex items-center gap-2">
      Height <input type="range" class="range range-xs w-24" min="120" max="900" step="20" bind:value={settings.height} />
      <span class="w-12 font-mono text-xs">{settings.height}px</span>
    </label>
    <SegmentedControl label="Fade" value={settings.fade} options={FADES} onchange={(fade) => (settings.fade = fade)} />
    <SegmentedControl label="Color" value={settings.color} options={COLORS} onchange={(color) => (settings.color = color)} />
    <button type="button" class="btn btn-ghost btn-xs" onclick={() => (settings = { ...DEFAULT_SETTINGS })}>Reset</button>
  </div>
</div>

<style>
  .pattern-backdrop {
    position: absolute;
    inset: 0 0 auto 0;
    z-index: -1;
  }

  .pattern-page {
    padding-bottom: 12rem;
  }

  .pattern-controls {
    position: fixed;
    inset: auto 1.5rem 1rem 1.5rem;
    z-index: 2;
    padding: 0.75rem 1rem;
    background: color-mix(in oklab, var(--color-base-100) 92%, transparent);
    border: 1px solid var(--color-base-300);
    border-radius: 1rem;
    box-shadow: 0 4px 16px rgb(0 0 0 / 0.08);
    backdrop-filter: blur(6px);
  }
</style>
