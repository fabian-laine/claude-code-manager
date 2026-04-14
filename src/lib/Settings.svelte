<script lang="ts">
  import { store } from "./store.svelte";
  import { isTauri, type RemoteStatus } from "./api";

  let { open = $bindable() }: { open: boolean } = $props();

  type Urls = { hostname: string | null; tailscale: string | null };

  let status = $state<RemoteStatus | null>(null);
  let urls = $state<Urls>({ hostname: null, tailscale: null });
  let busy = $state(false);
  let errorMsg = $state<string | null>(null);
  let successMsg = $state<string | null>(null);
  let port = $state(17890);
  let useHttps = $state(false);
  let copiedHint = $state<string | null>(null);

  async function refresh() {
    if (!isTauri) return;
    errorMsg = null;
    try {
      const api = await store.ensureApi();
      status = (await api.remoteStatus!()) as RemoteStatus;
      port = status.port || 17890;
      useHttps = status.https || status.cert_ready;
      urls = (await api.remoteUrls!()) as Urls;
    } catch (e) {
      errorMsg = String(e);
    }
  }

  $effect(() => {
    if (open) refresh();
  });

  async function toggle() {
    if (!status) return;
    busy = true;
    errorMsg = null;
    try {
      const api = await store.ensureApi();
      if (status.enabled) {
        await api.remoteStop!();
      } else {
        // HTTPS is mandatory — we only start when the cert is ready.
        if (!status.cert_ready) {
          errorMsg = "Generate the TLS certificate first.";
          busy = false;
          return;
        }
        await api.remoteStart!(port, true);
      }
      await refresh();
    } catch (e) {
      errorMsg = String(e);
    } finally {
      busy = false;
    }
  }

  async function rotate() {
    busy = true;
    try {
      const api = await store.ensureApi();
      await api.remoteRotateToken!();
      await refresh();
    } catch (e) {
      errorMsg = String(e);
    } finally {
      busy = false;
    }
  }

  async function generateCert() {
    if (!urls.tailscale) {
      errorMsg = "No Tailscale hostname detected on this machine.";
      return;
    }
    busy = true;
    errorMsg = null;
    successMsg = null;
    try {
      const api = await store.ensureApi();
      const result = await api.remoteGenerateTlsCert!(urls.tailscale);
      await refresh();
      useHttps = true;
      successMsg = `✓ Certificate generated for ${result.hostname}. Stop and restart the server to activate HTTPS.`;
      setTimeout(() => {
        if (successMsg?.startsWith("✓")) successMsg = null;
      }, 8000);
    } catch (e) {
      errorMsg = String(e);
    } finally {
      busy = false;
    }
  }

  function copy(text: string, hint: string) {
    navigator.clipboard.writeText(text);
    copiedHint = hint;
    setTimeout(() => {
      if (copiedHint === hint) copiedHint = null;
    }, 1500);
  }

  function buildUrl(host: string): string {
    const p = status?.port ?? port;
    const scheme = status?.https ? "https" : "http";
    return `${scheme}://${host}:${p}`;
  }

  const candidates = $derived.by(() => {
    if (!status) return [] as { label: string; url: string }[];
    const out: { label: string; url: string }[] = [];
    // If HTTPS is on, Tailscale is the only URL that matches the cert.
    if (status.https && status.cert_hostname) {
      out.push({ label: "Tailscale", url: buildUrl(status.cert_hostname) });
      return out;
    }
    if (urls.tailscale) {
      out.push({ label: "Tailscale", url: buildUrl(urls.tailscale) });
    }
    if (urls.hostname) {
      out.push({ label: "Local (LAN)", url: buildUrl(`${urls.hostname}.local`) });
      out.push({ label: "Hostname", url: buildUrl(urls.hostname) });
    }
    out.push({ label: "Loopback", url: buildUrl("localhost") });
    return out;
  });
</script>

{#if open}
  <div
    class="backdrop"
    onclick={() => (open = false)}
    role="presentation"
  ></div>
  <div class="modal" role="dialog" aria-modal="true">
    <header>
      <h2>Settings</h2>
      <button class="close" onclick={() => (open = false)} title="Close">×</button>
    </header>

    <section>
      <h3>Remote access</h3>
      <p class="desc">
        Use Claude Code Manager from your phone or another device through a
        browser. Your PC must stay on with the app running.
      </p>

      {#if !isTauri}
        <div class="muted">Only available from the desktop app.</div>
      {:else if !status}
        <div class="muted">Loading…</div>
      {:else}
        {@const tailscaleOk = !!urls.tailscale}
        {@const certOk = status.cert_ready}
        {@const serverOk = status.enabled}

        <!-- Setup checklist -->
        <ol class="steps">
          <li class:done={tailscaleOk}>
            <div class="bullet">{tailscaleOk ? "✓" : "1"}</div>
            <div class="step-body">
              <div class="step-title">Install Tailscale on this PC</div>
              <div class="step-desc">
                Creates a secure private network between your devices.
                {#if tailscaleOk}
                  Detected as <code>{urls.tailscale}</code>.
                {:else}
                  <br />
                  <code>sudo dnf install tailscale && sudo tailscale up</code>
                {/if}
              </div>
            </div>
          </li>

          <li class:done={tailscaleOk}>
            <div class="bullet">{tailscaleOk ? "✓" : "2"}</div>
            <div class="step-body">
              <div class="step-title">Allow cert generation without sudo</div>
              <div class="step-desc">
                Run this once so the app can call <code>tailscale cert</code>:
                <br />
                <code>sudo tailscale set --operator=$USER</code>
              </div>
            </div>
          </li>

          <li class:done={certOk}>
            <div class="bullet">{certOk ? "✓" : "3"}</div>
            <div class="step-body">
              <div class="step-title">Enable HTTPS in Tailscale admin</div>
              <div class="step-desc">
                Required to issue real certificates.
                <a
                  href="https://login.tailscale.com/admin/dns"
                  target="_blank"
                  rel="noreferrer">Open admin → DNS → HTTPS Certificates</a
                >.
              </div>
            </div>
          </li>

          <li class:done={certOk}>
            <div class="bullet">{certOk ? "✓" : "4"}</div>
            <div class="step-body">
              <div class="step-title">Generate the TLS certificate</div>
              <div class="step-desc">
                {#if certOk}
                  Certificate ready for <code>{status.cert_hostname}</code>.
                {:else}
                  The app runs <code>tailscale cert</code> for you and stores the
                  files in its data directory.
                {/if}
              </div>
              {#if tailscaleOk}
                <button
                  class="ghost small"
                  onclick={generateCert}
                  disabled={busy}
                >
                  {certOk ? "Regenerate" : "Generate"}
                </button>
              {/if}
            </div>
          </li>

          <li class:done={serverOk}>
            <div class="bullet">{serverOk ? "✓" : "5"}</div>
            <div class="step-body">
              <div class="step-title">Start the server</div>
              <div class="step-desc">
                {#if serverOk}
                  Running on port <code>{status.port}</code> (HTTPS).
                {:else if !certOk}
                  Waiting for the certificate.
                {:else}
                  Ready to go.
                {/if}
              </div>
              {#if !serverOk}
                <div class="form-row inline">
                  <label for="port-input">Port</label>
                  <input
                    id="port-input"
                    type="number"
                    min="1024"
                    max="65535"
                    bind:value={port}
                  />
                  <button
                    class="start small"
                    onclick={toggle}
                    disabled={busy || !certOk}
                  >
                    Start
                  </button>
                </div>
              {:else}
                <button
                  class="stop small"
                  onclick={toggle}
                  disabled={busy}
                >
                  Stop
                </button>
              {/if}
            </div>
          </li>
        </ol>

        {#if status.enabled}
          <div class="subsection">
            <h4>Access URL</h4>
            <p class="desc">
              Open one of these addresses from your phone (the network must be
              able to reach your PC — Tailscale is recommended for access from
              anywhere).
            </p>
            {#each candidates as c}
              <div class="url-row">
                <span class="url-label">{c.label}</span>
                <code>{c.url}</code>
                <button onclick={() => copy(c.url, c.label)}>
                  {copiedHint === c.label ? "✓" : "Copy"}
                </button>
              </div>
            {/each}
          </div>

          <div class="subsection">
            <h4>Access token</h4>
            <p class="desc">
              Enter this once on the remote browser. Keep it secret.
            </p>
            <div class="token-row">
              <code class="token">{status.token}</code>
              <button onclick={() => copy(status!.token, "token")}>
                {copiedHint === "token" ? "✓" : "Copy"}
              </button>
              <button class="ghost" onclick={rotate} disabled={busy}>
                Rotate
              </button>
            </div>
          </div>
        {/if}

        {#if successMsg}
          <div class="ok">{successMsg}</div>
        {/if}
        {#if errorMsg}
          <div class="err">{errorMsg}</div>
        {/if}
      {/if}
    </section>
  </div>
{/if}

<style>
  .backdrop {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.6);
    z-index: 100;
  }
  .modal {
    position: fixed;
    top: 50%;
    left: 50%;
    transform: translate(-50%, -50%);
    width: min(620px, 92vw);
    max-height: 88vh;
    overflow-y: auto;
    background: #14141a;
    border: 1px solid #2a2a33;
    border-radius: 12px;
    z-index: 101;
    box-shadow: 0 20px 60px rgba(0, 0, 0, 0.5);
  }
  header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 16px 20px;
    border-bottom: 1px solid #26262d;
  }
  h2 {
    margin: 0;
    font-size: 14px;
    color: #e8e8ee;
    text-transform: uppercase;
    letter-spacing: 0.08em;
  }
  .close {
    background: transparent;
    border: none;
    color: #9a9aa5;
    font-size: 22px;
    cursor: pointer;
    padding: 0 6px;
  }
  .close:hover {
    color: #e8e8ee;
  }
  section {
    padding: 18px 20px 22px 20px;
  }
  h3 {
    margin: 0 0 6px 0;
    font-size: 13px;
    color: #e8e8ee;
    text-transform: uppercase;
    letter-spacing: 0.08em;
  }
  h4 {
    margin: 0 0 6px 0;
    font-size: 11px;
    color: #9a9aa5;
    text-transform: uppercase;
    letter-spacing: 0.08em;
  }
  .desc {
    color: #8a8a95;
    font-size: 13px;
    margin: 6px 0 14px 0;
    line-height: 1.5;
  }
  .row {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 10px 0;
  }
  .status-dot {
    width: 10px;
    height: 10px;
    border-radius: 50%;
    background: #444;
  }
  .status-dot.on {
    background: #7aa870;
    box-shadow: 0 0 0 4px rgba(122, 168, 112, 0.18);
  }
  .status-text {
    flex: 1;
    color: #c8c8d2;
    font-size: 13px;
  }
  button {
    background: #c9a96e;
    color: #14141a;
    border: none;
    border-radius: 6px;
    padding: 7px 14px;
    font-weight: 600;
    cursor: pointer;
    font-size: 13px;
    font-family: inherit;
  }
  button:disabled {
    opacity: 0.4;
    cursor: not-allowed;
  }
  button.start {
    background: #7aa870;
  }
  button.stop {
    background: #b34545;
    color: #fff;
  }
  button.ghost {
    background: transparent;
    color: #9a9aa5;
    border: 1px solid #3a3a45;
  }
  button.ghost:hover:not(:disabled) {
    background: #1e1e25;
    color: #e8e8ee;
  }
  .steps {
    list-style: none;
    padding: 0;
    margin: 8px 0 4px 0;
    counter-reset: step;
  }
  .steps li {
    display: flex;
    gap: 14px;
    padding: 14px 0;
    border-bottom: 1px solid #1f1f26;
  }
  .steps li:last-child {
    border-bottom: none;
  }
  .bullet {
    flex-shrink: 0;
    width: 28px;
    height: 28px;
    border-radius: 50%;
    background: #1a1a22;
    border: 1px solid #2a2a33;
    display: flex;
    align-items: center;
    justify-content: center;
    color: #9a9aa5;
    font-size: 13px;
    font-weight: 600;
  }
  .steps li.done .bullet {
    background: #12241a;
    border-color: #2a5a3a;
    color: #86efac;
  }
  .step-body {
    flex: 1;
    min-width: 0;
  }
  .step-title {
    color: #e8e8ee;
    font-size: 13px;
    font-weight: 600;
    margin-bottom: 4px;
  }
  .steps li.done .step-title {
    color: #c8c8d2;
  }
  .step-desc {
    color: #8a8a95;
    font-size: 12px;
    line-height: 1.55;
  }
  .step-desc code {
    display: inline-block;
    background: #0e0e12;
    border: 1px solid #26262d;
    border-radius: 4px;
    padding: 1px 6px;
    font-family: "JetBrains Mono", ui-monospace, monospace;
    color: #c9a96e;
    font-size: 11px;
    margin: 2px 0;
  }
  .step-desc a {
    color: #6aa0d0;
  }
  button.small {
    padding: 5px 12px;
    font-size: 12px;
    margin-top: 10px;
  }
  .form-row {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 4px 0 12px 0;
  }
  .form-row.inline {
    padding: 0;
    margin-top: 10px;
  }
  .form-row label {
    font-size: 12px;
    color: #9a9aa5;
    width: 50px;
  }
  .form-row input[type="number"] {
    background: #1a1a22;
    border: 1px solid #26262d;
    color: #e8e8ee;
    padding: 6px 10px;
    border-radius: 6px;
    font-family: inherit;
    font-size: 13px;
    width: 100px;
  }
  .form-row input[type="checkbox"] {
    width: 16px;
    height: 16px;
    accent-color: #c9a96e;
  }
  .form-hint {
    font-size: 12px;
    color: #8a8a95;
  }
  .form-hint code {
    color: #c9a96e;
    font-family: "JetBrains Mono", ui-monospace, monospace;
  }
  .cert-row {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 10px 12px;
    background: #1a1a22;
    border: 1px solid #26262d;
    border-radius: 8px;
  }
  .cert-info {
    flex: 1;
    min-width: 0;
  }
  .cert-host {
    font-family: "JetBrains Mono", ui-monospace, monospace;
    font-size: 13px;
    color: #e8e8ee;
  }
  .cert-status {
    font-size: 11px;
    color: #7aa870;
    margin-top: 2px;
  }
  .desc a {
    color: #6aa0d0;
  }
  .desc code {
    color: #c9a96e;
    font-family: "JetBrains Mono", ui-monospace, monospace;
    font-size: 12px;
  }
  .subsection {
    margin-top: 18px;
    padding-top: 16px;
    border-top: 1px solid #1f1f26;
  }
  .url-row {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 6px 0;
  }
  .url-label {
    font-size: 11px;
    color: #6a6a75;
    text-transform: uppercase;
    letter-spacing: 0.06em;
    width: 90px;
  }
  .url-row code {
    flex: 1;
    background: #0e0e12;
    border: 1px solid #26262d;
    border-radius: 4px;
    padding: 6px 10px;
    font-family: "JetBrains Mono", ui-monospace, monospace;
    font-size: 12px;
    color: #c8c8d2;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .url-row button {
    background: transparent;
    color: #9a9aa5;
    border: 1px solid #3a3a45;
    font-weight: 500;
    padding: 5px 12px;
  }
  .url-row button:hover {
    background: #1e1e25;
    color: #e8e8ee;
  }
  .token-row {
    display: flex;
    align-items: center;
    gap: 10px;
  }
  .token {
    flex: 1;
    background: #0e0e12;
    border: 1px solid #26262d;
    border-radius: 4px;
    padding: 8px 12px;
    font-family: "JetBrains Mono", ui-monospace, monospace;
    font-size: 12px;
    color: #c9a96e;
    word-break: break-all;
  }
  .muted {
    color: #6a6a75;
    font-size: 13px;
    font-style: italic;
    padding: 10px 0;
  }
  .err {
    margin-top: 14px;
    padding: 10px 12px;
    background: #2a1a1a;
    border: 1px solid #5a2a2a;
    border-radius: 6px;
    color: #f87171;
    font-size: 12px;
  }
  .ok {
    margin-top: 14px;
    padding: 10px 12px;
    background: #12241a;
    border: 1px solid #2a5a3a;
    border-radius: 6px;
    color: #86efac;
    font-size: 12px;
  }
</style>
