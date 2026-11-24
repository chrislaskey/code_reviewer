<!-- One file in the New page's list: its name, edited when clicked until
     it is created, its size, what to do when it is a duplicate, and where
     it stands once Create runs. The page owns the row and the choices;
     this shows them and says what the person clicked. -->
<script>
  import { jt, msgid, ngettext, t } from "ttag";
  import { filesShowUrl } from "../../lib/routes.js";
  import { Badge, Button, FieldErrors, Input, Link, Panel, Text, TranslatedParts } from "../../lib/components/index.js";
  import { fileSize } from "../../lib/file_size.js";
  import { currentServerCommunitiesSingle } from "../../lib/current_server.svelte.js";

  /**
   * @typedef {"put" | "copy" | "skip"} Choice
   * @typedef {Object} Props
   * @property {any} row the page's row: file, name, existing, choice, status, progress, error, errors
   * @property {any} [earlier] the earlier row with the same bytes, when this is a duplicate of it
   * @property {Choice[]} offered the choices a duplicate has; none when it is not one
   * @property {Record<Choice, number>} rest how many rows still undecided offer each choice
   * @property {boolean} disabled
   * @property {(choice: Choice) => void} onchoose
   * @property {(choice: Choice) => void} onchooserest
   * @property {(name: string) => void} onrename
   * @property {() => void} onremove
   */

  /** @type {Props} */
  let { row, earlier, offered, rest, disabled, onchoose, onchooserest, onrename, onremove } = $props();

  const CHOICE_LABELS = { put: t`Put it in the communities chosen`, copy: t`Create a copy`, skip: t`Skip` };
  const STATUS_LABELS = { waiting: t`Waiting`, created: t`Created`, put: t`Put in`, skipped: t`Skipped` };

  let editing = $state(false);
  let editable = $derived(!disabled && (row.status === "ready" || row.status === "failed"));
  let chosen = $derived(offered.includes(row.choice) ? row.choice : null);

  /** @param {HTMLInputElement} element */
  function focus(element) {
    element.focus();
    element.select();
  }

  /** @param {KeyboardEvent} event */
  function keydown(event) {
    if (event.key === "Enter" || event.key === "Escape") {
      event.preventDefault();
      editing = false;
    }
  }
</script>

<Panel as="li" class="gap-2">
  <div class="flex items-center gap-3">
    <div class="flex min-w-0 grow flex-col">
      {#if editing && editable}
        <Input
          value={row.name}
          oninput={(event) => onrename(event.currentTarget.value)}
          onblur={() => (editing = false)}
          onkeydown={keydown}
          {@attach focus}
        />
      {:else if editable}
        <Button variant="ghost" size="sm" class="max-w-full justify-start truncate" title={t`Rename`} onclick={() => (editing = true)}>
          {row.name}
        </Button>
      {:else if row.created}
        <Link class="truncate" href={filesShowUrl(row.created)}>{row.name}</Link>
      {:else}
        <Text as="span" class="truncate">{row.name}</Text>
      {/if}
      <Text as="span" tone="muted" size="sm">{fileSize(row.file.size)}</Text>
      <FieldErrors errors={row.errors} fields={["name", "content_type", "size", "sha256"]} />
      {#if row.error && Object.keys(row.errors).length === 0}
        <Text as="span" tone="error" size="sm">{row.error}</Text>
      {/if}
    </div>
    {#if row.status === "uploading" && row.progress !== null}
      <progress class="progress w-24 shrink-0" value={row.progress} max="1"></progress>
    {:else if row.status === "failed"}
      <Badge tone="warning">{t`Failed`}</Badge>
    {:else if STATUS_LABELS[row.status]}
      <Badge>{STATUS_LABELS[row.status]}</Badge>
    {/if}
    {#if row.status === "ready"}
      <Button variant="ghost" size="xs" {disabled} onclick={onremove} aria-label={t`Take it off the list`}>
        {t`Remove`}
      </Button>
    {/if}
  </div>

  {#if offered.length > 0}
    <div class="flex flex-col gap-2">
      <Text size="sm">
        {#if row.existing && currentServerCommunitiesSingle()}
          {#snippet existingLink()}<Link href={filesShowUrl(row.existing.file.id)}>{row.existing.file.name}</Link>{/snippet}
          <TranslatedParts parts={jt`Already here, as ${existingLink}.`} />
        {:else if row.existing}
          {@const communityNames = row.existing.communities.map((community) => community.name).join(", ")}
          {#snippet existingLink()}<Link href={filesShowUrl(row.existing.file.id)}>{row.existing.file.name}</Link>{/snippet}
          <TranslatedParts parts={jt`Already here, as ${existingLink} in ${communityNames}.`} />
        {:else if earlier}
          {t`The same file as ${earlier.name}, earlier in this list.`}
        {/if}
      </Text>
      {#if editable}
        <div class="flex flex-wrap items-center gap-2">
          {#each offered as choice (choice)}
            <Button size="xs" variant={chosen === choice ? "primary" : "default"} onclick={() => onchoose(choice)}>
              {CHOICE_LABELS[choice]}
            </Button>
          {/each}
          {#if chosen && rest[chosen] > 0}
            <Button size="xs" variant="ghost" onclick={() => onchooserest(chosen)}>
              {@const count = rest[chosen]}
              {ngettext(msgid`The same for ${count} more`, `The same for ${count} more`, count)}
            </Button>
          {/if}
        </div>
      {/if}
    </div>
  {/if}
</Panel>
