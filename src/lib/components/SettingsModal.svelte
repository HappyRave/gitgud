<script lang="ts">
  import { onMount } from "svelte";
  import { configGetIdentity, configSetIdentity, toolDiagnostics, type ToolDiagnostics } from "$lib/api";
  import {
    prefs,
    setDensity,
    setDiffLayout,
    setTheme,
    toggleIgnoreWhitespace,
    type Density,
    type DiffLayout,
    type ThemeMode,
  } from "$lib/prefs.svelte";

  let {
    activeRepoId = null,
    activeRepoName = null,
    onclose,
  }: {
    activeRepoId?: number | null;
    activeRepoName?: string | null;
    onclose: () => void;
  } = $props();

  let scope = $state<"global" | "local">("global");
  let userName = $state("");
  let userEmail = $state("");
  let saving = $state(false);
  let savedMsg = $state<string | null>(null);
  let error = $state<string | null>(null);
  let diag = $state<ToolDiagnostics | null>(null);

  async function loadIdentity(nextScope: "global" | "local") {
    scope = nextScope;
    savedMsg = null;
    error = null;
    try {
      const cfg = await configGetIdentity(nextScope === "local" ? activeRepoId : null);
      userName = cfg.userName ?? "";
      userEmail = cfg.userEmail ?? "";
    } catch (err) {
      error = String(err);
    }
  }

  async function saveIdentity(event: SubmitEvent) {
    event.preventDefault();
    saving = true;
    savedMsg = null;
    error = null;
    try {
      await configSetIdentity(scope === "local" ? activeRepoId : null, userName, userEmail);
      savedMsg = scope === "local" ? `Saved for ${activeRepoName ?? "this repository"}` : "Saved global identity";
    } catch (err) {
      error = String(err);
    } finally {
      saving = false;
    }
  }

  onMount(() => {
    loadIdentity("global");
    toolDiagnostics().then(
      (d) => (diag = d),
      (err) => (error = String(err)),
    );
  });
</script>

<div class="overlay" role="presentation" onclick={(e) => e.target === e.currentTarget && onclose()}>
  <div
    class="modal"
    role="dialog"
    aria-modal="true"
    aria-label="Settings"
    tabindex="-1"
    onkeydown={(e) => e.key === "Escape" && (e.preventDefault(), onclose())}
  >
    <div class="hdr">
      <h2>Settings</h2>
      <button type="button" class="bare x" aria-label="Close" onclick={onclose}>✕</button>
    </div>

    {#if error}
      <p class="err" role="alert">{error}</p>
    {/if}

    <section class="sec">
      <div class="sec-h">
        <h3>Git Identity</h3>
        <div class="seg" role="group" aria-label="Identity scope">
          <button
            type="button"
            class="segb"
            class:on={scope === "global"}
            aria-pressed={scope === "global"}
            onclick={() => loadIdentity("global")}>Global</button
          >
          <button
            type="button"
            class="segb"
            class:on={scope === "local"}
            aria-pressed={scope === "local"}
            disabled={activeRepoId === null}
            onclick={() => loadIdentity("local")}>{activeRepoName ?? "Current repo"}</button
          >
        </div>
      </div>
      <form class="idform" onsubmit={saveIdentity}>
        <label>
          <span>user.name</span>
          <input type="text" bind:value={userName} placeholder="Your Name" />
        </label>
        <label>
          <span>user.email</span>
          <input type="email" bind:value={userEmail} placeholder="you@example.com" />
        </label>
        <div class="idact">
          <button type="submit" class="pri" disabled={saving}>{saving ? "Saving…" : "Save identity"}</button>
          {#if savedMsg}<span class="ok">{savedMsg}</span>{/if}
        </div>
      </form>
    </section>

    <section class="sec">
      <h3>Appearance & Diff</h3>
      <div class="prefs">
        <label>
          <span>Theme</span>
          <select value={prefs.theme} onchange={(e) => setTheme(e.currentTarget.value as ThemeMode)}>
            <option value="system">System</option>
            <option value="light">Light</option>
            <option value="dark">Dark</option>
          </select>
        </label>
        <label>
          <span>Density</span>
          <select value={prefs.density} onchange={(e) => setDensity(e.currentTarget.value as Density)}>
            <option value="standard">Standard (24px)</option>
            <option value="compact">Compact (20px)</option>
          </select>
        </label>
        <label>
          <span>Diff layout</span>
          <select value={prefs.diffLayout} onchange={(e) => setDiffLayout(e.currentTarget.value as DiffLayout)}>
            <option value="auto">Auto (split at ≥1400px)</option>
            <option value="unified">Unified</option>
            <option value="split">Side by side</option>
          </select>
        </label>
        <label class="chk">
          <input type="checkbox" checked={prefs.ignoreWhitespace} onchange={toggleIgnoreWhitespace} />
          <span>Ignore whitespace changes in diffs</span>
        </label>
      </div>
    </section>

    <section class="sec">
      <h3>Tools & Diagnostics</h3>
      {#if diag}
        <dl class="diag">
          <dt>Git version</dt>
          <dd>{diag.gitVersion}</dd>
          <dt>Git binary</dt>
          <dd class="mono">{diag.gitPath}</dd>
          <dt>Git LFS</dt>
          <dd>{diag.lfsVersion ?? "Not installed"}</dd>
        </dl>
      {:else}
        <p class="ab">Checking installed tools…</p>
      {/if}
    </section>
  </div>
</div>

<style>
  .overlay {
    position: fixed;
    inset: 0;
    background: rgba(15, 20, 28, 0.42);
    display: flex;
    justify-content: center;
    align-items: center;
    z-index: 40;
  }
  .modal {
    width: min(560px, 94vw);
    max-height: 86vh;
    overflow: auto;
    background: var(--panel);
    border: 1px solid var(--line);
    border-radius: 10px;
    padding: 16px 18px;
    box-shadow: 0 18px 46px rgba(0, 0, 0, 0.28);
  }
  .hdr {
    display: flex;
    align-items: center;
    justify-content: space-between;
    margin-bottom: 12px;
  }
  h2 {
    margin: 0;
    font: 600 15px var(--display);
  }
  h3 {
    margin: 0 0 8px;
    font: 600 12px var(--body);
    color: var(--muted);
    text-transform: uppercase;
    letter-spacing: 0.04em;
  }
  .sec {
    padding-top: 12px;
    margin-top: 12px;
    border-top: 1px solid var(--line);
  }
  .sec:first-of-type {
    border-top: 0;
    padding-top: 0;
    margin-top: 0;
  }
  .sec-h {
    display: flex;
    align-items: center;
    justify-content: space-between;
    margin-bottom: 8px;
  }
  .sec-h h3 {
    margin: 0;
  }
  .seg {
    display: inline-flex;
    border: 1px solid var(--line);
    border-radius: 5px;
    overflow: hidden;
  }
  .segb {
    border: 0;
    border-radius: 0;
    height: 22px;
    padding: 0 9px;
    font-size: 11px;
  }
  .segb.on {
    background: var(--sel);
    color: var(--l0);
    font-weight: 600;
  }
  .idform,
  .prefs {
    display: grid;
    gap: 8px;
  }
  .idform label,
  .prefs label:not(.chk) {
    display: grid;
    grid-template-columns: 96px 1fr;
    gap: 8px;
    align-items: center;
    font-size: 12px;
  }
  .chk {
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: 12px;
    padding-top: 2px;
  }
  input[type="text"],
  input[type="email"],
  select {
    height: 26px;
    padding: 0 8px;
    border: 1px solid var(--line);
    border-radius: 5px;
    background: var(--bg);
    color: var(--fg);
    font: inherit;
  }
  .idact {
    display: flex;
    align-items: center;
    gap: 10px;
  }
  .ok {
    color: var(--add-ink);
    font-size: 11.5px;
  }
  .err {
    margin: 0 0 10px;
    padding: 6px 8px;
    border-radius: 5px;
    background: var(--bad);
    color: var(--bad-ink);
    font-size: 12px;
  }
  .diag {
    display: grid;
    grid-template-columns: 96px 1fr;
    gap: 5px 10px;
    margin: 0;
    font-size: 12px;
  }
  .diag dt {
    color: var(--muted);
  }
  .diag dd {
    margin: 0;
    overflow-wrap: anywhere;
  }
  .mono {
    font-family: var(--mono);
    font-size: 11px;
  }
  .x {
    padding: 2px 6px;
  }
</style>
