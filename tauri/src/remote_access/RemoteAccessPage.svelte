<!-- Remote access. On the desktop: the switch for the listener on this
     computer's Tailscale address, where it can be reached, and the devices
     paired to use it, with a form to pair one that shows its pairing code
     once. On a phone: pairing with a computer by pasting that code. -->
<script>
  import { onDestroy, onMount } from "svelte";
  import AppNav from "../lib/components/AppNav.svelte";
  import Button from "../lib/components/Button.svelte";
  import Header from "../lib/components/Header.svelte";
  import StillWaves from "../lib/components/StillWaves.svelte";
  import TopBackgroundPattern from "../lib/components/TopBackgroundPattern.svelte";
  import {
    createPairedDevice,
    getAppPlatform,
    getPairingStatus,
    getRemoteAccessStatus,
    pairWithComputer,
    revokePairedDevice,
    setRemoteAccessEnabled,
    unpairFromComputer,
  } from "../lib/backend.js";
  import { log } from "../lib/log.svelte.js";

  // While remote access is on but not reachable yet, check again this often.
  const STARTING_REFRESH_MS = 5000;

  /** @type {"desktop" | "mobile" | null} */
  let platform = $state(null);
  let error = $state("");

  // Desktop
  let status = $state(null);
  let switching = $state(false);
  let newDeviceName = $state("");
  let pairing = $state(false);
  /** @type {{ device: { name: string }, pairing_code: string } | null} */
  let createdDevice = $state(null);
  let copied = $state(false);
  let codeElement = $state();

  // Mobile
  let pairingStatus = $state(null);
  let pastedCode = $state("");

  let refreshTimer = null;

  function show(promise, what) {
    error = "";
    return promise.catch((failure) => {
      error = String(failure);
      log(`${what} failed: ${failure}`);
      throw failure;
    });
  }

  async function refreshStatus() {
    status = await show(getRemoteAccessStatus(), "Reading remote access");
    scheduleRefreshWhileStarting();
  }

  function scheduleRefreshWhileStarting() {
    clearTimeout(refreshTimer);
    if (status?.enabled && status.url == null) {
      refreshTimer = setTimeout(() => refreshStatus().catch(() => {}), STARTING_REFRESH_MS);
    }
  }

  async function toggleRemoteAccess(event) {
    switching = true;
    try {
      status = await show(setRemoteAccessEnabled(event.currentTarget.checked), "Switching remote access");
      scheduleRefreshWhileStarting();
    } catch {
      // Shown above.
    } finally {
      switching = false;
    }
  }

  async function pairDevice(event) {
    event.preventDefault();
    pairing = true;
    copied = false;
    try {
      createdDevice = await show(createPairedDevice(newDeviceName), "Pairing a device");
      newDeviceName = "";
      await refreshStatus();
    } catch {
      // Shown above.
    } finally {
      pairing = false;
    }
  }

  async function revoke(device) {
    if (!confirm(`Unpair ${device.name}? It will stop working at once.`)) return;
    try {
      status = await show(revokePairedDevice(device.id), `Unpairing ${device.name}`);
    } catch {
      // Shown above.
    }
  }

  // The clipboard API can be unavailable in the webview, so fall back to
  // selecting the code for the user to copy.
  async function copyCode() {
    try {
      await navigator.clipboard.writeText(createdDevice.pairing_code);
      copied = true;
    } catch {
      codeElement?.select();
      log("Could not copy, the code is selected instead");
    }
  }

  async function pairPhone(event) {
    event.preventDefault();
    pairing = true;
    try {
      pairingStatus = await show(pairWithComputer(pastedCode), "Pairing");
      pastedCode = "";
    } catch {
      // Shown above.
    } finally {
      pairing = false;
    }
  }

  async function unpairPhone() {
    if (!confirm("Unpair this phone? You will need a new code from the computer to pair again.")) return;
    try {
      pairingStatus = await show(unpairFromComputer(), "Unpairing");
    } catch {
      // Shown above.
    }
  }

  function formatPairedAt(pairedAt) {
    return new Date(pairedAt).toLocaleString();
  }

  onMount(async () => {
    platform = await getAppPlatform();
    if (platform === "desktop") {
      refreshStatus().catch(() => {});
    } else {
      pairingStatus = await getPairingStatus();
    }
  });

  onDestroy(() => clearTimeout(refreshTimer));
</script>

<StillWaves rippleOnLoad />

<TopBackgroundPattern />

<AppNav current="remote" />

<main class="page with-waves">
  {#if platform === "desktop"}
    <Header>
      Remote access
      {#snippet subtitle()}
        Use this computer's sessions, reviews and traces from your phone, over Tailscale. Only devices you pair, signed
        in to your tailnet as you, can connect.
      {/snippet}
    </Header>

    {#if error}<p class="remote-error" role="alert">{error}</p>{/if}

    {#if status}
      <section class="panel remote-section">
        <label class="remote-switch">
          <input
            type="checkbox"
            class="toggle toggle-primary"
            checked={status.enabled}
            disabled={switching}
            onchange={toggleRemoteAccess}
          />
          Allow remote access
        </label>
        {#if !status.enabled}
          <p class="muted">Off: nothing listens on the tailnet.</p>
        {:else if status.url}
          <p>Reachable at <code>{status.url}</code></p>
        {:else if status.problem}
          <p class="remote-problem">Not reachable: {status.problem}</p>
        {:else}
          <p class="muted">Starting…</p>
        {/if}
      </section>

      <section class="panel remote-section">
        <h2>Paired devices</h2>
        {#if status.paired_devices.length === 0}
          <p class="muted">No devices paired yet.</p>
        {:else}
          <ul class="paired-devices">
            {#each status.paired_devices as device (device.id)}
              <li>
                <span>
                  <strong>{device.name}</strong>
                  <span class="muted">paired {formatPairedAt(device.paired_at)}</span>
                </span>
                <Button onclick={() => revoke(device)}>Unpair</Button>
              </li>
            {/each}
          </ul>
        {/if}

        {#if createdDevice}
          <div class="pairing-code">
            <p>
              The pairing code for <strong>{createdDevice.device.name}</strong>. Paste it into the app on the phone. It
              is shown only this once; anyone with it and access to your tailnet can use this computer as you.
            </p>
            <textarea bind:this={codeElement} readonly rows="3" spellcheck="false">{createdDevice.pairing_code}</textarea>
            <div class="pairing-code-actions">
              <Button onclick={copyCode}>{copied ? "Copied" : "Copy"}</Button>
              <Button onclick={() => (createdDevice = null)}>Done</Button>
            </div>
          </div>
        {:else}
          <form class="pair-device" onsubmit={pairDevice}>
            <input bind:value={newDeviceName} required maxlength="60" placeholder="Device name, such as My iPhone" />
            <Button variant="primary" type="submit" disabled={pairing || !status.url}>
              {pairing ? "Pairing…" : "Pair a device"}
            </Button>
          </form>
          {#if !status.url}<p class="muted">Turn on remote access to pair a device.</p>{/if}
        {/if}
      </section>
    {/if}
  {:else if platform === "mobile"}
    <Header>
      Pair with a computer
      {#snippet subtitle()}
        On the computer, open Remote, turn on remote access and pair a device, then paste its code here. This phone
        must be signed in to Tailscale as the same user.
      {/snippet}
    </Header>

    {#if error}<p class="remote-error" role="alert">{error}</p>{/if}

    {#if pairingStatus?.url}
      <section class="panel remote-section">
        <p>Paired with <code>{pairingStatus.url}</code></p>
        <Button onclick={unpairPhone}>Unpair</Button>
      </section>
    {:else if pairingStatus}
      <form class="panel remote-section" onsubmit={pairPhone}>
        <textarea bind:value={pastedCode} required rows="4" spellcheck="false" placeholder="cr1-…"></textarea>
        <Button variant="primary" type="submit" disabled={pairing}>{pairing ? "Pairing…" : "Pair"}</Button>
      </form>
    {/if}
  {/if}
</main>

<style>
  .remote-section {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: 0.75rem;
    padding: 1.25rem;
  }

  .remote-switch {
    display: flex;
    align-items: center;
    gap: 0.6rem;
    font-weight: 600;
    cursor: pointer;
  }

  .remote-problem,
  .remote-error {
    color: var(--color-error);
  }

  .paired-devices {
    width: 100%;
    margin: 0;
    padding: 0;
    list-style: none;
  }

  .paired-devices li {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 1rem;
    padding: 0.5rem 0;
  }

  .paired-devices li + li {
    border-top: 1px solid var(--color-base-300);
  }

  .paired-devices strong {
    margin-right: 0.5rem;
  }

  .pair-device {
    display: flex;
    flex-wrap: wrap;
    gap: 0.75rem;
    width: 100%;
  }

  .pair-device input {
    flex: 1;
    min-width: 12rem;
  }

  .pairing-code {
    display: flex;
    flex-direction: column;
    gap: 0.75rem;
    width: 100%;
  }

  textarea {
    width: 100%;
    font-family: ui-monospace, SFMono-Regular, Menlo, monospace;
    word-break: break-all;
  }

  .pairing-code-actions {
    display: flex;
    gap: 0.75rem;
  }
</style>
