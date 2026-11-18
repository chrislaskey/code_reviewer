<!-- The person's profile, at /profile: their name and their picture. A
     name is written where the page runs, so the app changes it offline
     and syncs it later; a picture goes straight to object storage, see
     avatarsUpload in lib/core/. A local user is told their account is
     only on this device; they and a trial user have no picture, which
     needs an account on the server. -->
<script>
  import FieldErrors from "../lib/FieldErrors.svelte";
  import { FormError, avatarsUpload, currentUserGet, currentUserUpdate } from "../lib/core/index.js";
  import { currentUserForget } from "../lib/current_user.js";
  import { accountsIndexUrl } from "../lib/routes.js";
  import { Alert, Button, ErrorMessage, FileInput, Heading, Input, Label, Text } from "../lib/components/index.js";
  import Page from "../lib/Page.svelte";

  /** @type {{name: string, kind?: string, avatar_url?: string | null} | null} */
  let currentUser = $state(null);
  /** @type {File | null} */
  let file = $state(null);
  /** @type {Record<string, string[]>} */
  let errors = $state({});
  let error = $state("");
  let busy = $state(false);
  let name = $state("");
  let nameError = $state("");
  let nameBusy = $state(false);
  /** From 0 to 1 while the file uploads, else null. */
  let progress = $state(null);

  async function load() {
    try {
      currentUser = await currentUserGet();
      name = currentUser?.name ?? "";
    } catch (problem) {
      error = String(problem);
    }
  }

  /** @param {Event & {currentTarget: HTMLInputElement}} event */
  function pick(event) {
    file = event.currentTarget.files?.[0] ?? null;
    errors = {};
    error = "";
  }

  /** @param {SubmitEvent} event */
  async function save(event) {
    event.preventDefault();
    if (!file) return;
    busy = true;
    errors = {};
    error = "";
    try {
      currentUser = await avatarsUpload(file, (fraction) => (progress = fraction));
      currentUserForget();
    } catch (problem) {
      if (problem instanceof FormError) errors = problem.errors;
      error = problem instanceof Error ? problem.message : String(problem);
    } finally {
      busy = false;
      progress = null;
    }
  }

  /** @param {SubmitEvent} event */
  async function saveName(event) {
    event.preventDefault();
    nameBusy = true;
    nameError = "";
    try {
      currentUser = await currentUserUpdate(name);
      name = currentUser?.name ?? "";
      currentUserForget();
    } catch (problem) {
      nameError = problem instanceof Error ? problem.message : String(problem);
    } finally {
      nameBusy = false;
    }
  }

  load();
</script>

<Page>
  <Heading level={1}>Profile</Heading>
  {#if currentUser}
    {#if currentUser.kind === "local"}
      <Alert tone="warning" class="flex flex-col items-start gap-2" data-local-user>
        <Text>
          <strong>This account is only on this device.</strong> Nothing is backed up: if the device is lost or reset,
          or the app is removed, what you made is gone. Sign up or sign in to keep it safe and use it on your other
          devices.
        </Text>
        <Button size="sm" href={accountsIndexUrl()}>Sign up or sign in</Button>
      </Alert>
    {/if}
    <div class="flex items-center gap-4">
      {#if currentUser.avatar_url}
        <img src={currentUser.avatar_url} alt={currentUser.name} class="size-20 rounded-full object-cover" />
      {:else}
        <div class="bg-base-300 flex size-20 items-center justify-center rounded-full text-2xl uppercase">
          {currentUser.name.slice(0, 1)}
        </div>
      {/if}
      <span>{currentUser.name}</span>
    </div>
    <form onsubmit={saveName} class="flex flex-col gap-3">
      <Label text="Name">
        <Input bind:value={name} autocomplete="name" maxlength="80" required />
      </Label>
      <ErrorMessage message={nameError} />
      <div>
        <Button variant="primary" type="submit" disabled={nameBusy || name.trim() === currentUser.name}>Save name</Button>
      </div>
    </form>
    {#if currentUser.kind === "full"}
      <form onsubmit={save} class="flex flex-col gap-3">
        <Label text="Picture">
          <FileInput
            accept="image/png,image/jpeg,image/gif,image/webp"
            onchange={pick}
            disabled={busy}
          />
          <FieldErrors {errors} fields={["content_type", "size"]} />
        </Label>
        {#if progress !== null}
          <progress class="progress w-full" value={progress} max="1"></progress>
        {/if}
        {#if error && Object.keys(errors).length === 0}
          <ErrorMessage message={error} />
        {/if}
        <div>
          <Button variant="primary" type="submit" disabled={busy || !file}>Save picture</Button>
        </div>
      </form>
    {/if}
  {:else if error}
    <ErrorMessage message={error} />
  {/if}
</Page>
