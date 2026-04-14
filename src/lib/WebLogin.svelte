<script lang="ts">
  import { getWebToken, setWebToken } from "./api";

  let {
    onLogin,
  }: {
    onLogin: () => void;
  } = $props();

  let token = $state("");
  let errorMsg = $state<string | null>(null);

  async function submit() {
    if (!token.trim()) return;
    errorMsg = null;
    try {
      // Probe the server with the provided token.
      const res = await fetch("/api/projects", {
        headers: { Authorization: `Bearer ${token.trim()}` },
      });
      if (res.status === 401) {
        errorMsg = "Invalid token.";
        return;
      }
      if (!res.ok) {
        errorMsg = `Server error: ${res.status}`;
        return;
      }
      setWebToken(token.trim());
      onLogin();
    } catch (e) {
      errorMsg = String(e);
    }
  }

  function onKey(e: KeyboardEvent) {
    if (e.key === "Enter") submit();
  }
</script>

<div class="login-wrap">
  <div class="login">
    <h1>Claude Code Manager</h1>
    <p class="sub">Remote access — enter your access token.</p>
    <input
      type="password"
      placeholder="Token"
      bind:value={token}
      onkeydown={onKey}
      autofocus
    />
    <button onclick={submit} disabled={!token.trim()}>Sign in</button>
    {#if errorMsg}
      <div class="err">{errorMsg}</div>
    {/if}
    <p class="hint">
      The token is shown in the desktop app → <strong>Settings → Remote
      access</strong>.
    </p>
  </div>
</div>

<style>
  .login-wrap {
    display: flex;
    align-items: center;
    justify-content: center;
    height: 100vh;
    width: 100vw;
    background: #0e0e12;
    padding: 20px;
  }
  .login {
    width: 100%;
    max-width: 400px;
    background: #14141a;
    border: 1px solid #26262d;
    border-radius: 12px;
    padding: 32px 28px;
    text-align: center;
  }
  h1 {
    margin: 0 0 6px 0;
    font-size: 18px;
    color: #e8e8ee;
  }
  .sub {
    margin: 0 0 24px 0;
    font-size: 13px;
    color: #8a8a95;
  }
  input {
    width: 100%;
    background: #1a1a22;
    border: 1px solid #26262d;
    color: #e8e8ee;
    padding: 12px 14px;
    border-radius: 8px;
    font-family: "JetBrains Mono", ui-monospace, monospace;
    font-size: 13px;
    margin-bottom: 12px;
    box-sizing: border-box;
    outline: none;
  }
  input:focus {
    border-color: #4a6a9a;
  }
  button {
    width: 100%;
    background: #c9a96e;
    color: #14141a;
    border: none;
    border-radius: 8px;
    padding: 12px;
    font-weight: 600;
    cursor: pointer;
    font-size: 14px;
    font-family: inherit;
  }
  button:disabled {
    opacity: 0.4;
    cursor: not-allowed;
  }
  .err {
    margin-top: 14px;
    padding: 10px;
    background: #2a1a1a;
    border: 1px solid #5a2a2a;
    border-radius: 6px;
    color: #f87171;
    font-size: 12px;
  }
  .hint {
    margin: 20px 0 0 0;
    font-size: 12px;
    color: #6a6a75;
    line-height: 1.5;
  }
</style>
