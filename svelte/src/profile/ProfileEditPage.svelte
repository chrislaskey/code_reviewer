<!-- The person's profile, at /profile/edit: their name and their picture. A
     name is written where the page runs, so the app changes it offline
     and syncs it later; a picture goes straight to object storage, see
     avatarsUpload in lib/core/. A local user is told their account is
     only on this device; they and a trial user have no picture, which
     needs an account on the server. Their language, when the pages are in
     more than one: a preference, synced, which reloads the page in it. -->
<script>
  import { t } from "ttag";
  import {
    FormError,
    accountsPreferencesUpdate,
    avatarsUpload,
    currentUserUpdate,
  } from "../lib/core/index.js";
  import { errorMessageOf } from "../lib/error_message.js";
  import { currentUser, currentUserLoad, currentUserReload } from "../lib/current_user.svelte.js";
  import { loaded } from "../lib/loaded.svelte.js";
  import { accountsIndexUrl } from "../lib/routes.js";
  import AccountsGuestNotice from "../lib/AccountsGuestNotice.svelte";
  import { TRANSLATIONS_LOCALES, translationsFollow, translationsLocaleShown } from "../lib/translations.js";
  import {
    Alert,
    Avatar,
    Button,
    ErrorMessage,
    FieldErrors,
    FileInput,
    Heading,
    Input,
    Label,
    Page,
    Select,
    Text,
  } from "../lib/components/index.js";

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
  let localeError = $state("");
  let localeBusy = $state(false);

  const profile = loaded(async () => {
    await currentUserLoad();
    name = currentUser.value?.name ?? "";
  });

  /** @param {string} locale */
  function languageName(locale) {
    const name = new Intl.DisplayNames([locale], { type: "language" }).of(locale) ?? locale;
    return name.charAt(0).toLocaleUpperCase(locale) + name.slice(1);
  }

  /** @param {Event & {currentTarget: HTMLSelectElement}} event */
  async function chooseLocale(event) {
    localeBusy = true;
    localeError = "";
    try {
      const preferences = await accountsPreferencesUpdate("locale", event.currentTarget.value || null);
      translationsFollow(preferences.locale);
    } catch (problem) {
      localeError = errorMessageOf(problem);
    } finally {
      localeBusy = false;
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
      await avatarsUpload(file, (fraction) => (progress = fraction));
      await currentUserReload();
    } catch (problem) {
      if (problem instanceof FormError) errors = problem.errors;
      error = errorMessageOf(problem);
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
      await currentUserUpdate(name);
      await currentUserReload();
      name = currentUser.value?.name ?? "";
    } catch (problem) {
      nameError = errorMessageOf(problem);
    } finally {
      nameBusy = false;
    }
  }
</script>

<Page>
  <Heading level={1}>{t`Profile`}</Heading>
  {#if currentUser.value}
    {#if currentUser.value.kind === "local"}
      <Alert tone="warning" class="flex flex-col items-start gap-2" data-local-user>
        <Text>
          <strong>{t`This account is only on this device.`}</strong>
          {t`Nothing is backed up: if the device is lost or reset, or the app is removed, what you made is gone. Sign up or log in to keep it safe and use it on your other devices.`}
        </Text>
        <Button size="sm" href={accountsIndexUrl()}>{t`Sign up or log in`}</Button>
      </Alert>
    {:else if currentUser.value.kind === "trial" && currentUser.value.trial_expires_at}
      <Alert tone="warning">
        <AccountsGuestNotice trialExpiresAt={currentUser.value.trial_expires_at} />
      </Alert>
    {/if}
    <div class="flex items-center gap-4">
      <Avatar user={currentUser.value} size="lg" />
      <span>{currentUser.value.name}</span>
    </div>
    <form onsubmit={saveName} class="flex flex-col gap-3">
      <Label text={t`Name`}>
        <Input bind:value={name} autocomplete="name" maxlength="80" required />
      </Label>
      <ErrorMessage message={nameError} />
      <div>
        <Button variant="primary" type="submit" disabled={nameBusy || name.trim() === currentUser.value.name}>{t`Save name`}</Button>
      </div>
    </form>
    {#if TRANSLATIONS_LOCALES.length > 1}
      {@const systemLanguage = languageName(translationsLocaleShown(null))}
      <Label text={t`Language`}>
        <Select value={currentUser.value.preferences?.locale ?? ""} onchange={chooseLocale} disabled={localeBusy}>
          <option value="">{t`Follow the system (${systemLanguage})`}</option>
          {#each TRANSLATIONS_LOCALES as locale (locale)}
            <!-- untranslated: each language in its own name -->
            <option value={locale} lang={locale}>{languageName(locale)}</option>
          {/each}
        </Select>
      </Label>
      <ErrorMessage message={localeError} />
    {/if}
    {#if currentUser.value.kind === "full"}
      <form onsubmit={save} class="flex flex-col gap-3">
        <Label text={t`Picture`}>
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
          <Button variant="primary" type="submit" disabled={busy || !file}>{t`Save picture`}</Button>
        </div>
      </form>
    {/if}
  {:else if profile.error}
    <ErrorMessage message={profile.error} />
  {/if}
</Page>
