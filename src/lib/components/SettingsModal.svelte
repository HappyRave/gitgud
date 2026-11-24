<script lang="ts">
  import { onMount } from "svelte";
  import {
    agentSetup,
    aiProviders,
    cacheStats,
    clearUnityCaches,
    configGetIdentity,
    configSetIdentity,
    hasCommitGraph,
    lfsCacheStats,
    lfsPrune,
    recentTimings,
    refreshIndex,
    toolDiagnostics,
    unityApplyRepoSetup,
    unityRepoSetupStatus,
    writeCommitGraph,
    type AiProviderStatus,
    type CacheStats,
    type LfsCacheStats,
    type TimingSample,
    type ToolDiagnostics,
    type UnityRepoSetupStatus,
  } from "$lib/api";
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
  import { modelCache, type ModelCacheStats } from "$lib/model/cache";

  let {
    activeRepoId = null,
    activeRepoName = null,
    onclose,
  }: {
    activeRepoId?: number | null;
    activeRepoName?: string | null;
    onclose: () => void;
  } = $props();

  let activeSection = $state("sec-identity");
  let scope = $state<"global" | "local">("global");
  let userName = $state("");
  let userEmail = $state("");
  let saving = $state(false);
  let savedMsg = $state<string | null>(null);
  let error = $state<string | null>(null);
  let diag = $state<ToolDiagnostics | null>(null);
  let commitGraph = $state<boolean | null>(null);
  let optimizingGraph = $state(false);
  let unitySetup = $state<UnityRepoSetupStatus | null>(null);
  let fixingUnitySetup = $state(false);
  let lfsStats = $state<LfsCacheStats | null>(null);
  let pruningLfs = $state(false);
  let aiStatus = $state<AiProviderStatus | null>(null);
  let agentTarget = $state<"claude" | "codex" | "cursor" | "generic">("claude");
  let agentWithHooks = $state(false);
  let writingAgentSetup = $state(false);
  let agentSetupPath = $state<string | null>(null);
  let timings = $state<TimingSample[]>([]);
  let refreshingIndex = $state(false);
  let indexRefreshedMsg = $state<string | null>(null);
  let caches = $state<CacheStats | null>(null);
  let models3d = $state<ModelCacheStats>(modelCache.stats());
  let clearingUnity = $state(false);

  const count = (n: number) => n.toLocaleString("en-US");

  async function clearUnity() {
    if (activeRepoId == null || clearingUnity) {
      return;
    }
    clearingUnity = true;
    error = null;
    try {
      caches = await clearUnityCaches(activeRepoId);
    } catch (err) {
      error = String(err);
    } finally {
      clearingUnity = false;
    }
  }

  function clearModels() {
    modelCache.clear();
    models3d = modelCache.stats();
  }

  function jumpTo(id: string) {
    activeSection = id;
    document.getElementById(id)?.scrollIntoView?.({ behavior: "smooth", block: "start" });
  }

  async function runRefreshIndex() {
    if (activeRepoId == null || refreshingIndex) {
      return;
    }
    refreshingIndex = true;
    error = null;
    indexRefreshedMsg = null;
    try {
      await refreshIndex(activeRepoId);
      indexRefreshedMsg = "Refreshed";
      timings = await recentTimings().catch(() => timings);
    } catch (err) {
      error = String(err);
    } finally {
      refreshingIndex = false;
    }
  }

  function formatBytes(bytes: number) {
    if (bytes >= 1024 * 1024 * 1024) {
      return `${(bytes / (1024 * 1024 * 1024)).toFixed(1)} GB`;
    }
    if (bytes >= 1024 * 1024) {
      return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
    }
    if (bytes >= 1024) {
      return `${(bytes / 1024).toFixed(1)} KB`;
    }
    return `${bytes} B`;
  }

  async function pruneLfs() {
    if (activeRepoId == null || pruningLfs) {
      return;
    }
    pruningLfs = true;
    error = null;
    try {
      lfsStats = await lfsPrune(activeRepoId);
    } catch (err) {
      error = String(err);
    } finally {
      pruningLfs = false;
    }
  }

  async function writeAgentConfig() {
    if (activeRepoId == null || writingAgentSetup) {
      return;
    }
    writingAgentSetup = true;
    error = null;
    agentSetupPath = null;
    try {
      agentSetupPath = await agentSetup(activeRepoId, agentTarget, agentTarget === "claude" && agentWithHooks);
    } catch (err) {
      error = String(err);
    } finally {
      writingAgentSetup = false;
    }
  }

  async function fixUnitySetup() {
    if (activeRepoId == null || fixingUnitySetup || !unitySetup) {
      return;
    }
    fixingUnitySetup = true;
    error = null;
    try {
      unitySetup = await unityApplyRepoSetup(
        activeRepoId,
        unitySetup.gitignoreMissing.length > 0,
        unitySetup.gitattributesMissing.length > 0,
      );
    } catch (err) {
      error = String(err);
    } finally {
      fixingUnitySetup = false;
    }
  }

  async function optimizeGraph() {
    if (activeRepoId == null || optimizingGraph) {
      return;
    }
    optimizingGraph = true;
    error = null;
    try {
      await writeCommitGraph(activeRepoId);
      commitGraph = true;
      caches = await cacheStats(activeRepoId);
    } catch (err) {
      error = String(err);
    } finally {
      optimizingGraph = false;
    }
  }

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
    aiProviders()
      .then((a) => (aiStatus = a))
      .catch(() => (aiStatus = null));
    recentTimings()
      .then((t) => (timings = t))
      .catch(() => (timings = []));
    if (activeRepoId != null) {
      hasCommitGraph(activeRepoId).then(
        (built) => (commitGraph = built),
        () => (commitGraph = null),
      );
      lfsCacheStats(activeRepoId).then(
        (s) => (lfsStats = s),
        () => (lfsStats = null),
      );
      cacheStats(activeRepoId).then(
        (s) => (caches = s),
        (err) => (error = String(err)),
      );
      unityRepoSetupStatus(activeRepoId).then(
        (s) => (unitySetup = s),
        () => (unitySetup = null),
      );
    }
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
      <div class="hdr-title">
        <h2>Settings</h2>
      </div>
      <button type="button" class="bare x" aria-label="Close" onclick={onclose}>✕</button>
    </div>

    {#if error}
      <p class="err" role="alert">{error}</p>
    {/if}

    <div class="layout">
      <nav class="rail" aria-label="Settings sections">
        <p class="rail-h">App</p>
        <button
          type="button"
          class="bare rail-btn"
          class:on={activeSection === "sec-identity"}
          onclick={() => jumpTo("sec-identity")}>Git Identity</button
        >
        <button
          type="button"
          class="bare rail-btn"
          class:on={activeSection === "sec-appearance"}
          onclick={() => jumpTo("sec-appearance")}>Appearance &amp; Diff</button
        >
        <button
          type="button"
          class="bare rail-btn"
          class:on={activeSection === "sec-tools"}
          onclick={() => jumpTo("sec-tools")}>Tools &amp; Diagnostics</button
        >
        {#if activeRepoId != null}
          <p class="rail-h" title={activeRepoName ?? undefined}>
            Repository{#if activeRepoName}&nbsp;· <span class="rail-repo">{activeRepoName}</span>{/if}
          </p>
          <button
            type="button"
            class="bare rail-btn"
            class:on={activeSection === "sec-caches"}
            onclick={() => jumpTo("sec-caches")}>Caches</button
          >
        {/if}
        {#if unitySetup?.isUnity}
          <button
            type="button"
            class="bare rail-btn"
            class:on={activeSection === "sec-unity"}
            onclick={() => jumpTo("sec-unity")}>Unity Setup</button
          >
        {/if}
        {#if activeRepoId != null}
          <button
            type="button"
            class="bare rail-btn"
            class:on={activeSection === "sec-agents"}
            onclick={() => jumpTo("sec-agents")}>Coding Agents</button
          >
        {/if}
      </nav>

      <div class="content">
        <p class="group-h">App</p>
        <section id="sec-identity" class="sec">
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

        <section id="sec-appearance" class="sec">
          <h3>Appearance &amp; Diff</h3>
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

        <section id="sec-tools" class="sec">
          <h3>Tools &amp; Diagnostics</h3>
          {#if diag}
            <dl class="diag">
              <dt>Git version</dt>
              <dd class="gcache">
                <span>{diag.gitVersion}</span>
                {#if diag.gitSource === "bundled"}
                  <span class="badge warn" title="Using bundled fallback Git (system Git missing or older than 2.45.2)"
                    >bundled</span
                  >
                {:else if diag.gitSource === "system"}
                  <span class="badge">system</span>
                {/if}
              </dd>
              <dt>Git binary</dt>
              <dd class="mono">{diag.gitPath}</dd>
              <dt>Git LFS</dt>
              <dd class="gcache">
                <span>{diag.lfsVersion ?? "Not installed"}</span>
                {#if diag.lfsVersion && diag.lfsOk === false}
                  <span class="badge warn" title="git-lfs 3.3 or newer is recommended">&lt; 3.3 floor</span>
                {/if}
              </dd>
              <dt>AI CLIs</dt>
              <dd>
                {#if aiStatus === null}
                  Checking…
                {:else if aiStatus.cliAvailable.length > 0}
                  {aiStatus.cliAvailable.join(", ")}
                {:else if aiStatus.configuredEnvKey}
                  API key env var detected
                {:else}
                  None detected on PATH
                {/if}
              </dd>
              {#if activeRepoId != null}
                <dt>Git index</dt>
                <dd class="gcache">
                  <button type="button" disabled={refreshingIndex} onclick={runRefreshIndex}>
                    {refreshingIndex ? "Refreshing…" : "Refresh index"}
                  </button>
                  {#if indexRefreshedMsg}<span class="ok">{indexRefreshedMsg}</span>{/if}
                </dd>
              {/if}
            </dl>
          {:else}
            <p class="ab">Checking installed tools…</p>
          {/if}
          {#if timings.length > 0}
            <details class="latency">
              <summary>Local latency ({timings.length} recent ops)</summary>
              <ul class="lat-list">
                {#each timings.slice(0, 10) as sample, i (`${sample.op}:${sample.at}:${i}`)}
                  <li><span class="lat-op">{sample.op}</span><span class="mono">{sample.elapsedMs} ms</span></li>
                {/each}
              </ul>
            </details>
          {/if}
        </section>

        {#if activeRepoId != null}
          <p class="group-h">
            Repository{#if activeRepoName}&nbsp;· <span class="rail-repo">{activeRepoName}</span>{/if}
          </p>
          <section id="sec-caches" class="sec" aria-label="Caches">
            <h3>Caches</h3>
            <p class="ab">What gitgud keeps to go faster. Clearing is safe: each rebuilds when next needed.</p>
            {#if caches}
              <dl class="diag caches">
                <dt>Unity references</dt>
                <dd>
                  <div class="gcache">
                    <span>
                      {count(caches.metaFiles)} .meta files · {count(caches.commitScans)} commits scanned · {count(caches.modelInfos)} models
                      read
                    </span>
                    <button type="button" disabled={clearingUnity} onclick={clearUnity}>
                      {clearingUnity ? "Clearing…" : "Clear"}
                    </button>
                  </div>
                  <span class="where" title={caches.guidIndexFile}
                    >Memory, index saved to <span class="mono">{caches.guidIndexFile}</span>
                    {caches.guidIndexBytes > 0 ? `(${formatBytes(caches.guidIndexBytes)})` : "(not written yet)"}</span
                  >
                </dd>
                <dt>3D models</dt>
                <dd>
                  <div class="gcache">
                    <span>
                      {count(models3d.models)} models · {count(models3d.materials)} materials · ~{formatBytes(models3d.bytes)}
                    </span>
                    <button type="button" disabled={models3d.models + models3d.materials === 0} onclick={clearModels}>Clear</button>
                  </div>
                  <span class="where">Memory, shared by every repo, until gitgud closes</span>
                </dd>
                <dt>Commit graph</dt>
                <dd>
                  <div class="gcache">
                    <span>{caches.commitGraphBytes === null ? "Not built" : formatBytes(caches.commitGraphBytes)}</span>
                    <button type="button" disabled={optimizingGraph} onclick={optimizeGraph}>
                      {optimizingGraph ? "Building…" : commitGraph ? "Rebuild" : "Build"}
                    </button>
                  </div>
                  <span class="where mono" title={caches.commitGraphPath}>{caches.commitGraphPath}</span>
                </dd>
                {#if lfsStats && (lfsStats.objectCount > 0 || diag?.lfsVersion)}
                  <dt>LFS objects</dt>
                  <dd>
                    <div class="gcache">
                      <span>
                        {count(lfsStats.objectCount)} object{lfsStats.objectCount === 1 ? "" : "s"} · {formatBytes(lfsStats.totalBytes)}
                        {#if lfsStats.prunableCount > 0}
                          · {count(lfsStats.prunableCount)} prunable ({formatBytes(lfsStats.prunableBytes)})
                        {/if}
                      </span>
                      <button type="button" disabled={pruningLfs || lfsStats.prunableCount === 0} onclick={pruneLfs}>
                        {pruningLfs ? "Pruning…" : "Prune"}
                      </button>
                    </div>
                    <span class="where mono" title={caches.lfsObjectsPath}>{caches.lfsObjectsPath}</span>
                  </dd>
                {/if}
              </dl>
            {:else}
              <p class="ab">Measuring caches…</p>
            {/if}
          </section>
        {/if}

        {#if unitySetup?.isUnity}
          <section id="sec-unity" class="sec" aria-label="Unity Project Setup">
            <h3>Unity Project Setup</h3>
            <dl class="diag">
              <dt>Serialization</dt>
              <dd>
                {unitySetup.forceTextOk
                  ? "Force Text ✓"
                  : `Mode ${unitySetup.serializationMode ?? "unknown"} — set Asset Serialization to Force Text in Unity`}
              </dd>
              <dt>.gitignore</dt>
              <dd>
                {unitySetup.gitignoreMissing.length === 0
                  ? "Standard Unity ignores present ✓"
                  : `Missing ${unitySetup.gitignoreMissing.join(", ")}`}
              </dd>
              <dt>.gitattributes</dt>
              <dd>
                {unitySetup.gitattributesMissing.length === 0
                  ? "Standard Unity rules present ✓"
                  : `Missing ${unitySetup.gitattributesMissing.length} standard rule(s)`}
              </dd>
            </dl>
            {#if unitySetup.gitignoreMissing.length > 0 || unitySetup.gitattributesMissing.length > 0}
              <div class="usetup-act">
                <button type="button" class="pri" disabled={fixingUnitySetup} onclick={fixUnitySetup}>
                  {fixingUnitySetup ? "Updating…" : "Fix Unity .gitignore / .gitattributes"}
                </button>
              </div>
            {/if}
          </section>
        {/if}

        {#if activeRepoId != null}
          <section id="sec-agents" class="sec" aria-label="Coding Agents">
            <h3>Coding Agents (CLI / MCP)</h3>
            <div class="agent-row">
              <label class="agent-lbl">
                <span>Target</span>
                <select aria-label="Agent target" bind:value={agentTarget}>
                  <option value="claude">Claude Code</option>
                  <option value="codex">Codex</option>
                  <option value="cursor">Cursor</option>
                  <option value="generic">Generic</option>
                </select>
              </label>
              <button type="button" disabled={writingAgentSetup} onclick={writeAgentConfig}>
                {writingAgentSetup ? "Writing…" : "Write agent instructions"}
              </button>
              {#if agentSetupPath}
                <span class="ok">Wrote {agentSetupPath}</span>
              {/if}
            </div>
            {#if agentTarget === "claude"}
              <label class="chk">
                <input type="checkbox" bind:checked={agentWithHooks} />
                <span>Install Claude Stop hook (.claude/settings.local.json)</span>
              </label>
            {/if}
            <p class="mcp-hint">MCP server: <code class="mono">gitgud-cli mcp</code></p>
          </section>
        {/if}
      </div>
    </div>
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
    width: min(740px, 94vw);
    max-height: 86vh;
    display: flex;
    flex-direction: column;
    overflow: hidden;
    background: var(--panel);
    border: 1px solid var(--line);
    border-radius: 10px;
    box-shadow: 0 18px 46px rgba(0, 0, 0, 0.28);
  }
  .hdr {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 12px 16px;
    border-bottom: 1px solid var(--line);
    flex: none;
  }
  .hdr-title {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  h2 {
    margin: 0;
    font: 600 15px var(--display);
  }
  .layout {
    display: grid;
    grid-template-columns: 168px minmax(0, 1fr);
    min-height: 0;
    flex: 1;
  }
  .rail {
    display: flex;
    flex-direction: column;
    gap: 2px;
    padding: 10px 8px;
    background: var(--bg);
    border-right: 1px solid var(--line);
  }
  .rail-h {
    margin: 10px 10px 4px;
    font: 600 10px var(--mono);
    letter-spacing: 0.06em;
    text-transform: uppercase;
    color: var(--muted);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .rail-h:first-child {
    margin-top: 0;
  }
  .rail-repo {
    color: var(--l0);
    text-transform: none;
    letter-spacing: 0;
  }
  .rail-btn {
    width: 100%;
    text-align: left;
    padding: 6px 10px;
    border-radius: 5px;
    font: 500 12px var(--body);
    color: var(--muted);
  }
  .rail-btn:hover {
    background: var(--hover);
    color: var(--fg);
  }
  .rail-btn.on {
    background: var(--sel);
    color: var(--fg);
    font-weight: 600;
  }
  .content {
    overflow-y: auto;
    padding: 14px 18px;
    min-width: 0;
  }
  h3 {
    margin: 0 0 8px;
    font: 600 11px var(--mono);
    color: var(--muted);
    text-transform: uppercase;
    letter-spacing: 0.05em;
  }
  .sec {
    padding-top: 14px;
    margin-top: 14px;
    border-top: 1px solid var(--line);
  }
  .sec:first-of-type,
  .group-h + .sec {
    border-top: 0;
    padding-top: 0;
    margin-top: 0;
  }
  /* Sticks while scrolling, so it's always clear whether a section is app-wide or this repo's. */
  .group-h {
    position: sticky;
    top: -14px;
    z-index: 1;
    margin: 24px -18px 14px;
    padding: 7px 18px;
    background: var(--bg);
    border-top: 1px solid var(--line);
    border-bottom: 1px solid var(--line);
    font: 600 10.5px var(--mono);
    letter-spacing: 0.06em;
    text-transform: uppercase;
    color: var(--muted);
  }
  .group-h:first-child {
    margin-top: -14px;
    border-top: 0;
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
    margin: 10px 16px 0;
    padding: 6px 8px;
    border-radius: 5px;
    background: var(--bad);
    color: var(--bad-ink);
    font-size: 12px;
  }
  .diag {
    display: grid;
    grid-template-columns: 96px 1fr;
    gap: 6px 10px;
    margin: 0;
    font-size: 12px;
    align-items: baseline;
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
  .gcache {
    display: flex;
    align-items: center;
    flex-wrap: wrap;
    gap: 6px 8px;
  }
  .caches dd {
    display: grid;
    gap: 2px;
  }
  .caches dd .gcache {
    justify-content: space-between;
  }
  .where {
    color: var(--muted);
    font-size: 10.5px;
  }
  .usetup-act {
    margin-top: 8px;
  }
  .agent-row {
    display: flex;
    align-items: center;
    flex-wrap: wrap;
    gap: 8px;
  }
  .agent-lbl {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    font-size: 12px;
  }
  .mcp-hint {
    margin: 8px 0 0;
    font-size: 11.5px;
    color: var(--muted);
  }
  .latency {
    margin-top: 10px;
    font-size: 11.5px;
  }
  .latency summary {
    cursor: default;
    color: var(--muted);
  }
  .lat-list {
    list-style: none;
    margin: 6px 0 0;
    padding: 0;
    display: grid;
    gap: 2px;
  }
  .lat-list li {
    display: flex;
    justify-content: space-between;
    gap: 12px;
  }
  .lat-op {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .x {
    padding: 2px 6px;
    border-radius: 4px;
    color: var(--muted);
  }
  .x:hover {
    background: var(--hover);
    color: var(--fg);
  }
  @media (max-width: 560px) {
    .layout {
      grid-template-columns: 1fr;
    }
    .rail {
      flex-direction: row;
      overflow-x: auto;
      border-right: 0;
      border-bottom: 1px solid var(--line);
    }
    .rail-btn {
      width: auto;
      white-space: nowrap;
    }
    .rail-h {
      display: none;
    }
  }
</style>
