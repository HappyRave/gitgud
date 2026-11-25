<script lang="ts">
  import { onMount } from "svelte";
  import {
    gitCheckRemote,
    gitConfigCompare,
    gitInstallSystem,
    gitInventory,
    gitLatest,
    gitSetChoice,
    gitUpdateSystem,
    pickGitExe,
    type ConfigRow,
    type GitChoice,
    type GitInstall,
    type GitInventory,
    type LatestGit,
    type RemoteCheck,
  } from "$lib/api";

  let { activeRepoId = null }: { activeRepoId?: number | null } = $props();

  type Mode = GitChoice["mode"];
  const MODES: { mode: Mode; label: string; title: string }[] = [
    { mode: "auto", label: "Auto", title: "System git when it is new enough, else the embedded one" },
    { mode: "system", label: "System", title: "The git on PATH" },
    { mode: "embedded", label: "Embedded", title: "The MinGit shipped with gitgud" },
    { mode: "custom", label: "Custom…", title: "Pick any git.exe" },
  ];

  let inv = $state<GitInventory | null>(null);
  let error = $state<string | null>(null);
  let switching = $state<Mode | null>(null);
  let latest = $state<LatestGit | null>(null);
  let checkingLatest = $state(false);
  let confirmUpdate = $state(false);
  let updating = $state(false);
  let updateMsg = $state<string | null>(null);
  let remote = $state<RemoteCheck | null>(null);
  let checkingRemote = $state(false);
  let config = $state<ConfigRow[] | null>(null);
  let configError = $state<string | null>(null);
  let showAllConfig = $state(false);

  const diffs = $derived(config?.filter((r) => r.differs).length ?? 0);
  const shownConfig = $derived(showAllConfig ? (config ?? []) : (config ?? []).filter((r) => r.differs));

  async function load() {
    error = null;
    try {
      inv = await gitInventory();
    } catch (err) {
      error = String(err);
    }
  }

  async function choose(mode: Mode) {
    if (switching) {
      return;
    }
    let choice: GitChoice;
    if (mode === "custom") {
      const path = await pickGitExe();
      if (!path) {
        return;
      }
      choice = { mode, path };
    } else if (mode === inv?.choice.mode) {
      return;
    } else {
      choice = { mode };
    }
    switching = mode;
    error = null;
    try {
      inv = await gitSetChoice(choice);
      remote = null;
    } catch (err) {
      error = String(err);
    } finally {
      switching = null;
    }
  }

  async function checkLatest() {
    checkingLatest = true;
    error = null;
    try {
      latest = await gitLatest();
    } catch (err) {
      error = String(err);
    } finally {
      checkingLatest = false;
    }
  }

  async function updateSystem() {
    confirmUpdate = false;
    updating = true;
    error = null;
    updateMsg = null;
    try {
      const res = await gitUpdateSystem();
      updateMsg =
        res.kind === "installerStarted"
          ? "The Git for Windows installer is running. Re-check when it's done."
          : res.message || "System git is up to date.";
    } catch (err) {
      error = String(err);
    } finally {
      updating = false;
    }
  }

  async function installSystem() {
    error = null;
    updateMsg = null;
    try {
      await gitInstallSystem();
      updateMsg = "winget is installing Git for Windows in its own window. Re-check when it's done.";
    } catch (err) {
      error = String(err);
    }
  }

  async function checkRemote() {
    if (activeRepoId == null) {
      return;
    }
    checkingRemote = true;
    error = null;
    try {
      remote = await gitCheckRemote(activeRepoId);
    } catch (err) {
      error = String(err);
    } finally {
      checkingRemote = false;
    }
  }

  async function loadConfig(e: Event) {
    if (!(e.currentTarget as HTMLDetailsElement).open || config) {
      return;
    }
    try {
      config = await gitConfigCompare();
    } catch (err) {
      configError = String(err);
    }
  }

  const inUse = (install: GitInstall | null) =>
    install != null && inv?.active != null && samePath(install.path, inv.active.path);
  const samePath = (a: string, b: string) => a.replaceAll("\\", "/").toLowerCase() === b.replaceAll("\\", "/").toLowerCase();
  const credentials = (i: GitInstall) =>
    i.credentialManager ? `GCM ${i.credentialManager}` : (i.credentialHelpers.at(-1) ?? "none");

  onMount(load);
</script>

{#snippet card(title: string, install: GitInstall | null, behind: boolean, missing: string)}
  <div class="card" class:active={inUse(install)} aria-label="{title} git">
    <div class="card-h">
      <span class="card-t">{title}</span>
      {#if inUse(install)}<span class="badge">in use</span>{/if}
      {#if install?.error && install.version}<span class="badge warn">too old</span>{/if}
      {#if behind}<span class="badge warn">update available</span>{/if}
    </div>
    {#if install}
      <p class="ver mono">{install.version ?? "—"}</p>
      <dl class="facts">
        <dt>Path</dt>
        <dd class="mono wrap">{#each install.path.split(/(?<=[\\/])/) as part}{part}<wbr />{/each}</dd>
        <dt>LFS</dt>
        <dd>
          {install.lfsVersion ?? "missing"}
          {#if install.lfsVersion && !install.lfsOk}<span class="badge warn">&lt; 3.3</span>{/if}
        </dd>
        <dt>Credentials</dt>
        <dd title={install.credentialHelpers.join(", ")}>{credentials(install)}</dd>
      </dl>
      {#if install.error}<p class="warn-line">{install.error}</p>{/if}
    {:else}
      <p class="none">{missing}</p>
    {/if}
    {#if title === "System"}
      <div class="card-act">
        {#if !install}
          <button type="button" onclick={installSystem}>Install Git for Windows</button>
        {:else if confirmUpdate}
          <span class="confirm">Runs Git for Windows' updater; open Git Bash windows close.</span>
          <button type="button" class="pri" onclick={updateSystem}>Update</button>
          <button type="button" onclick={() => (confirmUpdate = false)}>Cancel</button>
        {:else}
          <button type="button" disabled={updating} onclick={() => (confirmUpdate = true)}
            >{updating ? "Updating…" : "Update system git…"}</button
          >
        {/if}
      </div>
    {:else if title === "Embedded"}
      <p class="note">Ships with gitgud; updates with the app.</p>
    {/if}
  </div>
{/snippet}

<div class="sec-h">
  <h3>Git</h3>
  <div class="seg" role="group" aria-label="Git to use">
    {#each MODES as m (m.mode)}
      <button
        type="button"
        class="segb"
        class:on={inv?.choice.mode === m.mode}
        aria-pressed={inv?.choice.mode === m.mode}
        title={m.title}
        disabled={switching != null}
        onclick={() => choose(m.mode)}>{switching === m.mode ? "…" : m.label}</button
      >
    {/each}
  </div>
</div>

{#if error}<p class="err" role="alert">{error}</p>{/if}

{#if inv}
  {#if inv.active}
    <p class="running">
      Running <b class="mono">{inv.active.version}</b>
      <span class="badge">{inv.active.source === "bundled" ? "embedded" : inv.active.source}</span>
      {#if inv.choice.mode === "auto"}<span class="muted">· Auto picks the system git when it's {inv.minGit} or newer</span>{/if}
    </p>
  {:else}
    <p class="err" role="alert">No usable git: {inv.activeError}. Pick another one above.</p>
  {/if}

  <div class="cards">
    {@render card("System", inv.system, latest?.systemBehind ?? false, "No git on PATH.")}
    {@render card("Embedded", inv.embedded, latest?.embeddedBehind ?? false, "Not shipped with this build.")}
    {#if inv.choice.mode === "custom"}
      {@render card("Custom", inv.custom, false, "")}
    {/if}
  </div>

  {#if updateMsg}<p class="ok">{updateMsg}</p>{/if}

  <div class="tools">
    <button type="button" disabled={checkingLatest} onclick={checkLatest}
      >{checkingLatest ? "Checking…" : "Check for updates"}</button
    >
    {#if latest}<span class="muted">Latest Git for Windows: <b class="mono">{latest.version}</b></span>{/if}
    <button type="button" onclick={load}>Re-check</button>
    {#if activeRepoId != null}
      <button type="button" disabled={checkingRemote} onclick={checkRemote}
        >{checkingRemote ? "Reaching remote…" : "Check remote access"}</button
      >
    {/if}
  </div>
  {#if remote}
    <p class={remote.ok ? "ok" : "warn-line"}>
      {#if remote.ok}
        ✓ Reached <b>{remote.remote}</b> in {remote.elapsedMs} ms with stored credentials.
      {:else}
        ✗ Couldn't reach <b>{remote.remote}</b> without a prompt: {remote.message}
      {/if}
      <span class="mono muted">{remote.url}</span>
    </p>
  {/if}

  {#if inv.system && inv.embedded}
    <details class="config" ontoggle={loadConfig}>
      <summary>
        Config: system vs embedded{#if config}&nbsp;· {diffs === 0 ? "identical" : `${diffs} differ`}{/if}
      </summary>
      {#if configError}
        <p class="warn-line">{configError}</p>
      {:else if !config}
        <p class="muted">Reading…</p>
      {:else}
        <p class="hint">
          Both read your global <span class="mono">~/.gitconfig</span> and each repo's own config. Only their system-level
          files differ; the embedded one includes Git for Windows' when it is installed.
        </p>
        <label class="chk"><input type="checkbox" bind:checked={showAllConfig} /> Show all keys</label>
        {#if shownConfig.length > 0}
          <table>
            <thead><tr><th>Key</th><th>Scope</th><th>System</th><th>Embedded</th></tr></thead>
            <tbody>
              {#each shownConfig as row (row.scope + row.key)}
                <tr class:differs={row.differs}>
                  <td class="mono">{row.key}</td>
                  <td>{row.scope}</td>
                  <td class="mono">{row.system.join(", ") || "—"}</td>
                  <td class="mono">{row.embedded.join(", ") || "—"}</td>
                </tr>
              {/each}
            </tbody>
          </table>
        {/if}
      {/if}
    </details>
  {/if}

  {#if inv.settingsPath}
    <p class="hint">Saved in <span class="mono">{inv.settingsPath}</span>; the gitgud CLI and MCP server use it too.</p>
  {/if}
{:else if !error}
  <p class="muted">Looking for git…</p>
{/if}

<style>
  .sec-h {
    display: flex;
    align-items: center;
    justify-content: space-between;
    margin-bottom: 8px;
  }
  h3 {
    margin: 0;
    font: 600 11px var(--mono);
    color: var(--muted);
    text-transform: uppercase;
    letter-spacing: 0.05em;
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
  .running {
    display: flex;
    align-items: baseline;
    flex-wrap: wrap;
    gap: 6px;
    margin: 0 0 8px;
    font-size: 12px;
  }
  .cards {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(220px, 1fr));
    gap: 8px;
  }
  .card {
    display: flex;
    flex-direction: column;
    gap: 4px;
    min-width: 0;
    margin: 0;
    padding: 8px 10px;
    border: 1px solid var(--line);
    border-radius: 7px;
    background: var(--bg);
  }
  .card.active {
    border-color: var(--l0);
  }
  .card-h {
    display: flex;
    align-items: center;
    gap: 6px;
  }
  .card-t {
    font: 600 12px var(--body);
    margin-right: auto;
  }
  .ver {
    margin: 0;
    font-size: 14px;
    font-weight: 600;
  }
  .facts {
    display: grid;
    grid-template-columns: 74px minmax(0, 1fr);
    gap: 2px 8px;
    margin: 0;
    font-size: 11.5px;
  }
  .facts dt {
    color: var(--muted);
  }
  .facts dd {
    margin: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .facts dd.wrap {
    white-space: normal;
    overflow-wrap: anywhere;
  }
  .none,
  .note {
    margin: 0;
    font-size: 11.5px;
    color: var(--muted);
  }
  .card-act {
    display: flex;
    align-items: center;
    flex-wrap: wrap;
    gap: 6px;
    margin-top: auto;
    padding-top: 4px;
  }
  .confirm {
    font-size: 11px;
    color: var(--muted);
    flex-basis: 100%;
  }
  .tools {
    display: flex;
    align-items: center;
    flex-wrap: wrap;
    gap: 8px;
    margin-top: 10px;
    font-size: 12px;
  }
  .config {
    margin-top: 10px;
    font-size: 12px;
  }
  .config summary {
    cursor: default;
    color: var(--muted);
  }
  table {
    width: 100%;
    margin-top: 6px;
    border-collapse: collapse;
    font-size: 11px;
    table-layout: fixed;
  }
  th {
    text-align: left;
    font-weight: 600;
    color: var(--muted);
    border-bottom: 1px solid var(--line);
    padding: 2px 4px;
  }
  td {
    padding: 2px 4px;
    overflow-wrap: anywhere;
    vertical-align: top;
  }
  tr.differs td {
    background: var(--hover);
  }
  .chk {
    display: flex;
    align-items: center;
    gap: 6px;
    margin-top: 6px;
  }
  .hint {
    margin: 8px 0 0;
    font-size: 11px;
    color: var(--muted);
  }
  .muted {
    color: var(--muted);
  }
  .mono {
    font-family: var(--mono);
    font-size: 11px;
  }
  .ok {
    margin: 8px 0 0;
    color: var(--add-ink);
    font-size: 11.5px;
  }
  .warn-line {
    margin: 4px 0 0;
    color: var(--bad-ink);
    font-size: 11.5px;
    overflow-wrap: anywhere;
  }
  .err {
    margin: 0 0 8px;
    padding: 6px 8px;
    border-radius: 5px;
    background: var(--bad);
    color: var(--bad-ink);
    font-size: 12px;
  }
</style>
