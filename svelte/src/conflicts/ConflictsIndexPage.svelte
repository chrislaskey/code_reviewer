<!-- The changes made here that are not on the server, at /conflicts:
     those waiting to reach it, which the next sync sends, and those it
     did not keep, each to reapply, or to try again when the server
     refused it outright, or let go -->
<script>
  import { t } from "ttag";
  import { dateTimeFormat } from "../lib/formats.js";
  import {
    conflictsDelete,
    conflictsList,
    conflictsReapply,
    conflictsTryAgain,
    pendingChangesList,
  } from "../lib/core/index.js";
  import { loaded } from "../lib/loaded.svelte.js";
  import { errorMessageOf } from "../lib/error_message.js";
  import { postsIndexUrl } from "../lib/routes.js";
  import { Button, ErrorMessage, Heading, Link, Page, Panel, Text } from "../lib/components/index.js";

  const changes = loaded(
    async () => {
      const [pending, conflicts] = await Promise.all([pendingChangesList(), conflictsList()]);
      return { pending, conflicts };
    },
    { reloadOnSync: true },
  );
  let pending = $derived(changes.value?.pending ?? []);
  let conflicts = $derived(changes.value?.conflicts ?? []);
  let error = $state("");

  /** @param {() => Promise<unknown>} resolve */
  async function resolveThen(resolve) {
    error = "";
    try {
      await resolve();
      await changes.reload();
    } catch (problem) {
      error = errorMessageOf(problem);
    }
  }

  /** @param {unknown} value */
  const shown = (value) => (value === null || value === undefined ? "" : String(value));
</script>

<Page>
  <Heading level={1}>{t`Changes not on the server`}</Heading>

  <ErrorMessage message={error || changes.error} />

  {#if changes.value && pending.length === 0 && conflicts.length === 0}
    <Text tone="muted">{t`Every change reached the server.`}</Text>
  {/if}

  {#if pending.length > 0}
    <Heading>{t`Waiting to reach the server`}</Heading>
    <ul class="flex flex-col gap-3">
      {#each pending as change (change.id)}
        <Panel as="li">
          <Text tone="muted">{change.about ?? change.table} · {change.kind} · {dateTimeFormat(change.made_at)}</Text>
          {#each Object.entries(change.changes) as [name, value] (name)}
            <Text><strong>{name}</strong></Text>
            <Text class="whitespace-pre-wrap">{shown(value)}</Text>
          {/each}
        </Panel>
      {/each}
    </ul>
  {/if}

  {#if conflicts.length > 0}
    <Heading>{t`Not kept by the server`}</Heading>
  {/if}
  <ul class="flex flex-col gap-3">
    {#each conflicts as conflict (conflict.id)}
      <Panel as="li">
        <Text tone="muted">{conflict.about ?? t`Something that is gone`} · {dateTimeFormat(conflict.made_at)}</Text>
        {#if conflict.reason}
          <Text tone="error" size="sm">{conflict.reason}</Text>
        {/if}
        {#if conflict.resolve === "try_again"}
          <Text>{t`The server did not take this. It is still on this device.`}</Text>
        {:else}
          {#each conflict.fields as field (field.name)}
            <Text><strong>{t`Your change`}</strong></Text>
            <Text class="whitespace-pre-wrap">{shown(field.yours)}</Text>
            <Text><strong>{t`Now on the server`}</strong></Text>
            <Text class="whitespace-pre-wrap">{shown(field.now)}</Text>
          {/each}
        {/if}
        <div class="flex gap-2">
          {#if conflict.resolve === "reapply"}
            <Button size="sm" onclick={() => resolveThen(() => conflictsReapply(conflict.id))}>
              {t`Reapply`}
            </Button>
          {:else if conflict.resolve === "try_again"}
            <Button size="sm" onclick={() => resolveThen(() => conflictsTryAgain(conflict.id))}>
              {t`Try again`}
            </Button>
          {/if}
          <Button size="sm" variant="ghost" onclick={() => resolveThen(() => conflictsDelete(conflict.id))}>
            {t`Let it go`}
          </Button>
        </div>
      </Panel>
    {/each}
  </ul>

  <Link href={postsIndexUrl()}>{t`Back to posts`}</Link>
</Page>
