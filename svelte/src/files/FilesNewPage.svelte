<!-- Creating files, at /files/new: the person picks or drops one file or
     many, up to 5,000, into a list. Each keeps its own name, editable,
     and all go into the communities chosen, the current one to start.
     A file with the same bytes as one the person sees, or as an earlier
     one in the list, waits for a choice: put the existing file in the
     communities chosen, create a copy, or skip; one choice can be given
     to the rest. Create runs the list, each file on its own, and says
     what came of each; the failed ones can be tried again. A list of
     one that is created shows the file. The person stays on the page
     while the files go. See docs/design/files-create-many.md. -->
<script>
  import { msgid, ngettext, t } from "ttag";
  import {
    FormError,
    filesCreate,
    filesCreateAtOnce,
    filesDuplicatesList,
    filesSha256,
    filesUpdate,
  } from "../lib/core/index.js";
  import { errorMessageOf } from "../lib/error_message.js";
  import { currentCommunityLoad } from "../lib/current_community.svelte.js";
  import { navigateToUrl } from "../lib/router.js";
  import { filesIndexUrl, filesShowUrl } from "../lib/routes.js";
  import { numberFormat } from "../lib/formats.js";
  import { Alert, Button, FileInput, Heading, Label, Link, Page, Text } from "../lib/components/index.js";
  import FilesCommunitiesField from "./components/FilesCommunitiesField.svelte";
  import FilesNewRow from "./components/FilesNewRow.svelte";

  const MAX_FILES = 5000;
  const maxFiles = numberFormat(MAX_FILES);
  /** Checksums asked about in one request, the most the server takes. */
  const CHECKSUMS_PER_REQUEST = 500;
  const CHECKSUMS_AT_ONCE = 4;

  /**
   * @typedef {"put" | "copy" | "skip"} Choice
   * @typedef {"ready" | "waiting" | "uploading" | "created" | "put" | "skipped" | "failed"} Status
   * @typedef {Object} Row
   * @property {number} key
   * @property {File} file
   * @property {string} name
   * @property {string | null} sha256 once worked out
   * @property {any} existing the newest file the person sees with these bytes
   * @property {Choice | null} choice
   * @property {Status} status
   * @property {number | null} progress
   * @property {string | null} created the file made or put in, once it is
   * @property {string} error
   * @property {Record<string, string[]>} errors
   */

  /** @type {Row[]} */
  let rows = $state([]);
  let nextKey = 0;
  /** @type {string[]} */
  let communityIds = $state([]);
  let notice = $state("");
  /** Rows whose duplicates are being asked for, and how many of them have their checksum. */
  let asking = $state(0);
  let hashed = $state(0);
  /**
   * The rows as shown: failed first, then duplicates, then the rest, each
   * in the order picked. Arranged when the list is checked, changed or run,
   * not as each row changes, which with thousands would redraw them all.
   * @type {Row[]}
   */
  let shown = $state.raw([]);
  let busy = $state(false);
  let dragging = $state(false);
  /** How many rows the running Create took, and how many are done. */
  let run = $state({ total: 0, done: 0 });

  /** The first row with each checksum, which the later ones duplicate. */
  let firstBySha256 = $derived.by(() => {
    const first = new Map();
    for (const row of rows) {
      if (row.sha256 && !first.has(row.sha256)) first.set(row.sha256, row);
    }
    return first;
  });

  /** @param {Row} row */
  function earlierOf(row) {
    if (row.existing || !row.sha256) return undefined;
    const first = firstBySha256.get(row.sha256);
    return first === row ? undefined : first;
  }

  /**
   * @param {Row} row
   * @returns {Choice[]}
   */
  function offeredOf(row) {
    if (row.existing) {
      const inAlready = row.existing.communities.map((community) => community.id);
      const missing = communityIds.some((id) => !inAlready.includes(id));
      return missing ? ["put", "copy", "skip"] : ["copy", "skip"];
    }
    return earlierOf(row) ? ["copy", "skip"] : [];
  }

  /** @param {Row} row */
  function undecided(row) {
    const offered = offeredOf(row);
    return offered.length > 0 && !offered.includes(row.choice);
  }

  let open = $derived(rows.filter((row) => row.status === "ready"));
  let failed = $derived(rows.filter((row) => row.status === "failed"));
  let toDecide = $derived(open.filter(undecided).length + failed.filter(undecided).length);
  let rest = $derived.by(() => {
    const counts = { put: 0, copy: 0, skip: 0 };
    for (const row of rows) {
      if ((row.status !== "ready" && row.status !== "failed") || !undecided(row)) continue;
      for (const choice of offeredOf(row)) counts[choice] += 1;
    }
    return counts;
  });
  let counts = $derived.by(() => {
    const counted = { created: 0, put: 0, skipped: 0, failed: 0 };
    for (const row of rows) if (row.status in counted) counted[row.status] += 1;
    return counted;
  });
  let ready = $derived(open.length > 0 && asking === 0 && toDecide === 0 && communityIds.length > 0);

  function arrange() {
    const rank = (/** @type {Row} */ row) => (row.status === "failed" ? 0 : offeredOf(row).length > 0 ? 1 : 2);
    shown = [...rows].sort((a, b) => rank(a) - rank(b) || a.key - b.key);
  }

  /** @param {FileList | File[] | null | undefined} picked */
  function add(picked) {
    const files = [...(picked ?? [])];
    if (files.length === 0) return;
    const room = MAX_FILES - rows.length;
    const leftOut = files.length - room;
    notice =
      leftOut > 0
        ? ngettext(
            msgid`A list holds ${maxFiles} files; ${leftOut} was left out.`,
            `A list holds ${maxFiles} files; ${leftOut} were left out.`,
            leftOut,
          )
        : "";
    const start = rows.length;
    rows.push(
      ...files.slice(0, Math.max(room, 0)).map((file) => ({
        key: nextKey++,
        file,
        name: file.name,
        sha256: null,
        existing: null,
        choice: null,
        status: /** @type {Status} */ ("ready"),
        progress: null,
        created: null,
        error: "",
        errors: {},
      })),
    );
    arrange();
    check(rows.slice(start));
  }

  /**
   * Works out each row's checksum, then asks for the files the person
   * sees with them, a few hundred a request, and writes what it found to
   * the rows at once at the end. When the asking fails the rows are
   * created as copies, as for one file.
   * @param {Row[]} fresh
   */
  async function check(fresh) {
    asking += fresh.length;
    /** @type {Map<Row, string>} */
    const sha256s = new Map();
    /** @type {Map<Row, string>} */
    const problems = new Map();
    /** @type {Map<string, any>} */
    const newest = new Map();
    await eachAtOnce(fresh, CHECKSUMS_AT_ONCE, async (row) => {
      try {
        sha256s.set(row, await filesSha256(row.file));
      } catch (problem) {
        problems.set(row, errorMessageOf(problem));
      }
      hashed += 1;
    });
    try {
      const unique = [...new Set(sha256s.values())];
      for (let index = 0; index < unique.length; index += CHECKSUMS_PER_REQUEST) {
        const found = await filesDuplicatesList(unique.slice(index, index + CHECKSUMS_PER_REQUEST));
        for (const same of found) if (!newest.has(same.file.sha256)) newest.set(same.file.sha256, same);
      }
    } catch {
      // Creating a copy is still fine
    }
    for (const [row, sha256] of sha256s) {
      row.sha256 = sha256;
      row.existing = newest.get(sha256) ?? null;
    }
    for (const [row, problem] of problems) {
      row.status = "failed";
      row.error = problem;
    }
    asking -= fresh.length;
    hashed -= fresh.length;
    arrange();
  }

  /**
   * Runs `work` on each item, `atOnce` at a time, in order.
   * @template T
   * @param {T[]} items
   * @param {number} atOnce
   * @param {(item: T) => Promise<void>} work
   */
  async function eachAtOnce(items, atOnce, work) {
    let next = 0;
    async function worker() {
      while (next < items.length) await work(items[next++]);
    }
    await Promise.all(Array.from({ length: Math.min(atOnce, items.length) }, worker));
  }

  /** @param {Row} row */
  function remove(row) {
    rows = rows.filter((other) => other !== row);
    arrange();
  }

  /**
   * Gives the choice to every row still undecided that offers it.
   * @param {Choice} choice
   */
  function chooseRest(choice) {
    for (const row of rows) {
      if ((row.status === "ready" || row.status === "failed") && undecided(row) && offeredOf(row).includes(choice)) {
        row.choice = choice;
      }
    }
  }

  /** @param {Row} row */
  async function createOne(row) {
    row.error = "";
    row.errors = {};
    const choice = offeredOf(row).length > 0 ? row.choice : null;
    try {
      if (choice === "skip") {
        row.status = "skipped";
      } else if (choice === "put") {
        row.status = "uploading";
        const existing = row.existing;
        const inAlready = existing.communities.map((community) => community.id);
        const ids = [...inAlready, ...communityIds.filter((id) => !inAlready.includes(id))];
        await filesUpdate(existing.file.id, existing.file.name, ids);
        row.created = existing.file.id;
        row.status = "put";
      } else {
        row.status = "uploading";
        const file = await filesCreate(row.file, row.name, communityIds, (fraction) => (row.progress = fraction), row.sha256);
        row.created = file.id;
        row.status = "created";
      }
    } catch (problem) {
      if (problem instanceof FormError) row.errors = problem.errors;
      row.error = errorMessageOf(problem);
      row.status = "failed";
    } finally {
      row.progress = null;
      run.done += 1;
    }
  }

  /** @param {Row[]} chosen the rows to run, in the order picked */
  async function createRows(chosen) {
    if (chosen.length === 0) return;
    busy = true;
    run = { total: chosen.length, done: 0 };
    for (const row of chosen) row.status = "waiting";
    try {
      await eachAtOnce(chosen, filesCreateAtOnce, createOne);
    } finally {
      busy = false;
      arrange();
    }
    const [only] = chosen;
    if (rows.length === 1 && only.created && (only.status === "created" || only.status === "put")) {
      await navigateToUrl(filesShowUrl(only.created));
    }
  }

  /** @param {SubmitEvent} event */
  function save(event) {
    event.preventDefault();
    if (ready) createRows(open);
  }

  /** @param {Event & {currentTarget: HTMLInputElement}} event */
  function pick(event) {
    add(event.currentTarget.files);
    event.currentTarget.value = "";
  }

  /** @param {DragEvent} event */
  function drop(event) {
    event.preventDefault();
    dragging = false;
    if (!busy) add(event.dataTransfer?.files);
  }

  /** @param {DragEvent} event */
  function dragover(event) {
    event.preventDefault();
    dragging = !busy;
  }

  /** @param {BeforeUnloadEvent} event */
  function beforeunload(event) {
    if (busy) event.preventDefault();
  }

  async function load() {
    try {
      const current = await currentCommunityLoad();
      if (current && communityIds.length === 0) communityIds = [current.id];
    } catch {
      // The person picks one
    }
  }

  load();
</script>

<svelte:window onbeforeunload={beforeunload} />

<Page>
  <Heading level={1}>{t`New files`}</Heading>
  <form onsubmit={save} class="flex flex-col gap-3">
    <div
      role="region"
      aria-label={t`Files to create`}
      class={["flex flex-col gap-3 rounded-box border-2 border-dashed p-3", dragging ? "border-primary" : "border-transparent"]}
      ondragover={dragover}
      ondragleave={() => (dragging = false)}
      ondrop={drop}
    >
      <Label text={t`Files`}>
        <FileInput multiple onchange={pick} disabled={busy} />
      </Label>
      <Text tone="muted" size="sm">{t`Pick one file or many, or drop them here, up to ${maxFiles}.`}</Text>
      {#if notice}
        <Alert>{notice}</Alert>
      {/if}
      {#if rows.length > 0}
        <ul class="flex flex-col gap-2">
          {#each shown as row (row.key)}
            <FilesNewRow
              {row}
              earlier={earlierOf(row)}
              offered={offeredOf(row)}
              {rest}
              disabled={busy}
              onchoose={(choice) => (row.choice = choice)}
              onchooserest={chooseRest}
              onrename={(name) => (row.name = name)}
              onremove={() => remove(row)}
            />
          {/each}
        </ul>
      {/if}
    </div>
    <FilesCommunitiesField bind:communityIds disabled={busy} />
    <!-- While files go, nothing here counts the whole list, which would
         run again for every row that changes. -->
    {#if busy}
      <Text tone="muted">{t`Creating ${run.done} of ${run.total}. Stay on this page until they are done.`}</Text>
      <progress class="progress w-full" value={run.done} max={run.total}></progress>
      <div>
        <Button variant="primary" disabled>{t`Creating…`}</Button>
      </div>
    {:else}
      {#if counts.created + counts.put + counts.skipped + counts.failed > 0}
        <Text>
          {t`${counts.created} created, ${counts.put} put in, ${counts.skipped} skipped, ${counts.failed} failed.`}
        </Text>
      {/if}
      {#if asking > 0}
        {@const hashedCount = numberFormat(hashed)}
        {@const askingCount = numberFormat(asking)}
        <Text tone="muted" size="sm">
          {t`Looking for files already here… ${hashedCount} of ${askingCount} read.`}
        </Text>
      {:else if toDecide > 0}
        {@const toDecideCount = numberFormat(toDecide)}
        <Text tone="muted" size="sm">
          {ngettext(
            msgid`One file is already here; choose what to do with it.`,
            `${toDecideCount} files are already here; choose what to do with each.`,
            toDecide,
          )}
        </Text>
      {/if}
      <div class="flex flex-wrap gap-2">
        {#if open.length > 0 || rows.length === 0}
          <Button variant="primary" type="submit" disabled={!ready}>
            {#if open.length === 0}
              {t`Create files`}
            {:else}
              {@const openCount = numberFormat(open.length)}
              {ngettext(msgid`Create file`, `Create ${openCount} files`, open.length)}
            {/if}
          </Button>
        {/if}
        {#if failed.length > 0}
          <Button disabled={toDecide > 0 || communityIds.length === 0} onclick={() => createRows(failed)}>{t`Retry failed`}</Button>
        {/if}
      </div>
    {/if}
  </form>
  <Link href={filesIndexUrl()}>{t`Back to files`}</Link>
</Page>
