<!-- The changes made here that are not on the server, at /conflicts:
     those waiting to reach it, which the next sync sends, and those it
     did not keep, each to reapply, or to try again when the server
     refused it outright, or let go -->
<script>
  import {
    conflictsLetGo,
    conflictsList,
    conflictsReapply,
    conflictsTryAgain,
    onSynced,
    pendingChangesList,
  } from "../lib/core/index.js";
  import { postsIndexUrl } from "../lib/routes.js";
  import { Button, ErrorMessage, Heading, Link, Panel, Text } from "../lib/components/index.js";
  import Page from "../lib/Page.svelte";

  let pending = $state([]);
  let conflicts = $state([]);
  let error = $state("");

  async function load() {
    try {
      [pending, conflicts] = await Promise.all([pendingChangesList(), conflictsList()]);
    } catch (problem) {
      error = String(problem);
    }
  }

  /** @param {() => Promise<unknown>} resolve */
  async function resolveThen(resolve) {
    error = "";
    try {
      await resolve();
      await load();
    } catch (problem) {
      error = String(problem);
    }
  }

  /** @param {unknown} value */
  const shown = (value) => (value === null || value === undefined ? "" : String(value));

  $effect(() => onSynced(load));
  load();
</script>

<Page>
  <Heading level={1}>Changes not on the server</Heading>

  <ErrorMessage message={error} />

  {#if pending.length === 0 && conflicts.length === 0 && !error}
    <Text tone="muted">Every change reached the server.</Text>
  {/if}

  {#if pending.length > 0}
    <Heading>Waiting to reach the server</Heading>
    <ul class="flex flex-col gap-3">
      {#each pending as change (change.id)}
        <Panel as="li">
          <Text tone="muted">{change.about ?? change.table} · {change.kind} · {change.made_at}</Text>
          {#each Object.entries(change.changes) as [name, value] (name)}
            <Text><strong>{name}</strong></Text>
            <Text class="whitespace-pre-wrap">{shown(value)}</Text>
          {/each}
        </Panel>
      {/each}
    </ul>
  {/if}

  {#if conflicts.length > 0}
    <Heading>Not kept by the server</Heading>
  {/if}
  <ul class="flex flex-col gap-3">
    {#each conflicts as conflict (conflict.id)}
      <Panel as="li">
        <Text tone="muted">{conflict.about ?? "Something that is gone"} · {conflict.made_at}</Text>
        {#if conflict.reason}
          <Text tone="error" size="sm">{conflict.reason}</Text>
        {/if}
        {#if conflict.resolve === "try_again"}
          <Text>The server did not take this. It is still on this device.</Text>
        {:else}
          {#each conflict.fields as field (field.name)}
            <Text><strong>Your change</strong></Text>
            <Text class="whitespace-pre-wrap">{shown(field.yours)}</Text>
            <Text><strong>Now on the server</strong></Text>
            <Text class="whitespace-pre-wrap">{shown(field.now)}</Text>
          {/each}
        {/if}
        <div class="flex gap-2">
          {#if conflict.resolve === "reapply"}
            <Button size="sm" onclick={() => resolveThen(() => conflictsReapply(conflict.id))}>
              Reapply
            </Button>
          {:else if conflict.resolve === "try_again"}
            <Button size="sm" onclick={() => resolveThen(() => conflictsTryAgain(conflict.id))}>
              Try again
            </Button>
          {/if}
          <Button size="sm" variant="ghost" onclick={() => resolveThen(() => conflictsLetGo(conflict.id))}>
            Let it go
          </Button>
        </div>
      </Panel>
    {/each}
  </ul>

  <Link href={postsIndexUrl()}>Back to posts</Link>
</Page>
