<script lang="ts">
  import {
    branches as readBranches,
    compare,
    compareFileDiff,
    compareUnityDiff,
    predictMerge,
    stashes as readStashes,
    tags as readTags,
    type Branches,
    type Change,
    type ComparePoint,
    type CompareMode,
    type Comparison,
    type FileDiff,
    type StashEntry,
    type TagInfo,
    type UnityYamlDiff,
  } from "$lib/api";
  import { allFolders, buildTree, fileSides, imageAt, kinds, pointLabel, treeRows } from "$lib/compare/tree";
  import { imageUrl, isImage } from "$lib/images";
  import { prefs, viewModeFor } from "$lib/prefs.svelte";
  import { age } from "$lib/time";
  import { isUnityYaml } from "$lib/unity";
  import { viewerFor } from "$lib/viewers/route";
  import DiffView from "./DiffView.svelte";
  import FilePreview from "./FilePreview.svelte";
  import LoadingBar from "./LoadingBar.svelte";
  import RefPicker from "./RefPicker.svelte";
  import UnityDiffView from "./UnityDiffView.svelte";
  import ViewSwitch from "./ViewSwitch.svelte";

  let {
    repoId,
    a: initialA,
    b: initialB,
    mode: initialMode = null,
    currentBranch = null,
    busy = false,
    ontake,
    onclose,
  }: {
    repoId: number;
    a: ComparePoint;
    b: ComparePoint;
    /** null: "what B brings" for two refs, as decided (R12 Q2). */
    mode?: CompareMode | null;
    /** The checked-out branch: B is predicted as a merge into it when A is this branch. */
    currentBranch?: string | null;
    busy?: boolean;
    /** "Take B's version of this file" (R12 Q3). */
    ontake: (rev: string, path: string) => void;
    onclose: () => void;
  } = $props();

  const LETTER: Record<Change, string> = { added: "A", modified: "M", deleted: "D", typeChange: "T" };
  /** Comparisons up to this many files open with every folder unfolded. */
  const OPEN_ALL = 300;
  /** Flat mode draws at most this many rows. */
  const FLAT_MAX = 2000;

  // svelte-ignore state_referenced_locally
  let a = $state<ComparePoint>(initialA);
  // svelte-ignore state_referenced_locally
  let b = $state<ComparePoint>(initialB);
  // svelte-ignore state_referenced_locally
  let mode = $state<CompareMode>(initialMode ?? "brings");

  let result = $state<Comparison | null>(null);
  let error = $state<string | null>(null);
  let loading = $state(false);
  let conflicts = $state<string[] | null>(null);

  let refs = $state<{ branches: Branches | null; tags: TagInfo[]; stashes: StashEntry[] }>({ branches: null, tags: [], stashes: [] });
  let flat = $state(false);
  let openFolders = $state<Set<string>>(new Set());
  let file = $state<string | null>(null);
  let diff = $state<FileDiff | null>(null);
  let unityDiff = $state<UnityYamlDiff | null>(null);
  let fileError = $state<string | null>(null);

  $effect(() => {
    const id = repoId;
    Promise.all([readBranches(id), readTags(id), readStashes(id)]).then(
      ([br, tg, st]) => (refs = { branches: br, tags: tg, stashes: st }),
      (err) => console.error("compare pickers:", err),
    );
  });

  $effect(() => {
    const key = JSON.stringify([repoId, a, b, mode]);
    const [ra, rb, rm] = [$state.snapshot(a), $state.snapshot(b), mode];
    loading = true;
    error = null;
    compare(repoId, ra, rb, rm).then(
      (c) => {
        if (key !== JSON.stringify([repoId, a, b, mode])) {
          return;
        }
        loading = false;
        result = c;
        const tree = buildTree(c.files);
        openFolders = new Set(c.files.length <= OPEN_ALL ? allFolders(tree) : []);
        if (!file || !c.files.some((f) => f.path === file)) {
          file = c.files.length <= OPEN_ALL ? (c.files[0]?.path ?? null) : null;
        }
      },
      (err) => {
        if (key === JSON.stringify([repoId, a, b, mode])) {
          loading = false;
          result = null;
          error = String(err);
        }
      },
    );
  });

  // "Merging would conflict": only when B is a branch about to come into the current one.
  $effect(() => {
    conflicts = null;
    const branch = currentBranch;
    if (!branch || a.kind !== "rev" || b.kind !== "rev" || a.rev !== branch || b.rev === branch) {
      return;
    }
    const target = b.rev;
    predictMerge(repoId, target).then(
      (files) => {
        if (b.kind === "rev" && b.rev === target) {
          conflicts = files;
        }
      },
      (err) => console.error("merge prediction:", err),
    );
  });

  const tree = $derived(result ? buildTree(result.files) : null);
  const rows = $derived(tree ? treeRows(tree, openFolders) : []);
  const fileKinds = $derived(result ? kinds(result.files) : []);
  const sides = $derived(result ? fileSides(result, a, b) : null);
  const merged = $derived(!!result && result.mode === "brings" && result.files.length === 0 && result.onlyB.count === 0);
  const noBase = $derived(!!result && mode === "brings" && result.base === null);
  const forcedSnapshot = $derived(!!result && mode === "brings" && result.mode === "snapshot" && a.kind === "worktree");

  const viewMode = $derived(file ? viewModeFor(file) : "source");
  const showVisual = $derived(!!file && viewMode === "visual" && viewerFor(file) !== null);
  const structural = $derived(!!file && isUnityYaml(file) && viewMode === "structure");

  $effect(() => {
    const path = file;
    const s = sides;
    const ws = prefs.ignoreWhitespace;
    const wantUnity = structural;
    diff = null;
    unityDiff = null;
    fileError = null;
    if (!path || !s) {
      return;
    }
    const stale = () => file !== path || sides !== s || prefs.ignoreWhitespace !== ws;
    const loadRaw = () =>
      compareFileDiff(repoId, s.from, s.to, path, ws).then(
        (d) => !stale() && (diff = d),
        (err) => !stale() && (fileError = String(err)),
      );
    if (wantUnity) {
      compareUnityDiff(repoId, s.from, s.to, path).then(
        (u) => !stale() && (unityDiff = u),
        () => !stale() && loadRaw(),
      );
    } else {
      loadRaw();
    }
  });

  function swap() {
    [a, b] = [b, a];
  }

  function toggleFolder(path: string) {
    const next = new Set(openFolders);
    if (next.has(path)) {
      next.delete(path);
    } else {
      next.add(path);
    }
    openFolders = next;
  }

  function onKeydown(e: KeyboardEvent) {
    const t = e.target as HTMLElement | null;
    if (e.key === "Escape" && !(t instanceof HTMLInputElement)) {
      e.preventDefault();
      onclose();
    }
  }

  const sideName = (p: ComparePoint) => pointLabel(p);
  const n = (x: number) => x.toLocaleString("en-US");
</script>

<svelte:window onkeydown={onKeydown} />

<section class="compare" aria-label="Compare">
  <div class="bar">
    <RefPicker side="A" value={a} {...refs} onpick={(p) => (a = p)} />
    <button type="button" class="sm" title="Swap A and B" aria-label="Swap A and B" onclick={swap}>⇄</button>
    <RefPicker side="B" value={b} {...refs} onpick={(p) => (b = p)} />
    <span class="seg" role="radiogroup" aria-label="Compare mode">
      <button
        type="button"
        role="radio"
        aria-checked={mode === "brings"}
        class:on={mode === "brings"}
        title="B against where A and B split: what merging B would bring"
        onclick={() => (mode = "brings")}>What B brings</button
      >
      <button
        type="button"
        role="radio"
        aria-checked={mode === "snapshot"}
        class:on={mode === "snapshot"}
        title="The two snapshots against each other, changes on both sides included"
        onclick={() => (mode = "snapshot")}>Snapshot difference</button
      >
    </span>
    {#if conflicts}
      <span class="pred" class:bad={conflicts.length > 0} role="status">
        {#if conflicts.length}
          Merging {sideName(b)} would conflict in {conflicts.length} {conflicts.length === 1 ? "file" : "files"}
        {:else}
          {sideName(b)} merges cleanly
        {/if}
      </span>
    {/if}
    <span class="sp"></span>
    <button type="button" class="sm" onclick={onclose}>Close <span class="k">Esc</span></button>
  </div>

  {#if loading && !result}
    <p class="note"><LoadingBar label="Comparing…" /></p>
  {/if}
  {#if error}
    <p class="note bad" role="alert">{error}</p>
  {/if}

  {#if result}
    {#if merged}
      <div class="info" role="status">
        <b>{sideName(b)} brings nothing: it's fully merged into {sideName(a)}.</b>
        <button type="button" class="sm" onclick={() => (mode = "snapshot")}>Show the snapshot difference</button>
      </div>
    {:else if noBase}
      <div class="info warn" role="status"><b>These two have no commit in common.</b> Showing the snapshot difference.</div>
    {:else if forcedSnapshot}
      <div class="info" role="status">The working copy on the A side has no history: showing the snapshot difference.</div>
    {/if}

    <div class="body" class:loading>
      <div class="col commits" role="region" aria-label="Commits">
        {#each [{ tag: "B", list: result.onlyB, other: a }, { tag: "A", list: result.onlyA, other: b }] as side (side.tag)}
          <h3 class="sub">Only in {side.tag} · {n(side.list.count)}</h3>
          {#if side.list.count === 0}
            <p class="none">Nothing: {sideName(side.tag === "B" ? b : a)} has no commits {sideName(side.other)} lacks.</p>
          {:else}
            <ul aria-label="Only in {side.tag}">
              {#each side.list.commits as c (c.id)}
                <li title="{c.shortId} · {c.author} · {new Date(c.time * 1000).toLocaleString()}">
                  <span class="ab {side.tag.toLowerCase()}">{side.tag}</span>
                  <span class="s">{c.subject}</span>
                  <span class="who">{age(c.time)}</span>
                </li>
              {/each}
              {#if side.list.count > side.list.commits.length}
                <li class="more">+ {n(side.list.count - side.list.commits.length)} older</li>
              {/if}
            </ul>
          {/if}
        {/each}
      </div>

      <div class="col files" role="region" aria-label="Files">
        <h3 class="sub">
          {n(result.files.length)}
          {result.files.length === 1 ? "file" : "files"}
          <span class="r seg" role="radiogroup" aria-label="File list">
            <button type="button" role="radio" aria-checked={!flat} class:on={!flat} onclick={() => (flat = false)}>Folders</button>
            <button type="button" role="radio" aria-checked={flat} class:on={flat} onclick={() => (flat = true)}>Flat</button>
          </span>
        </h3>
        {#if fileKinds.length}
          <div class="kinds">
            {#each fileKinds.slice(0, 6) as k (k.ext)}
              <span class="chip">{n(k.count)} {k.ext}</span>
            {/each}
            {#if fileKinds.length > 6}
              <span class="chip">+ {fileKinds.length - 6} kinds</span>
            {/if}
          </div>
        {/if}
        <ul class="tree" aria-label="Changed files">
          {#if flat}
            {#each result.files.slice(0, FLAT_MAX) as f (f.path)}
              {@const cut = f.path.lastIndexOf("/") + 1}
              <li>
                <button type="button" class="row" class:current={f.path === file} aria-pressed={f.path === file} title={f.path} onclick={() => (file = f.path)}>
                  <span class="badge l-{LETTER[f.change]}">{LETTER[f.change]}</span><span class="p"
                    ><span class="dir">{f.path.slice(0, cut)}</span>{f.path.slice(cut)}</span
                  >
                </button>
              </li>
            {/each}
            {#if result.files.length > FLAT_MAX}
              <li class="more">Showing {n(FLAT_MAX)} of {n(result.files.length)}: use Folders for the rest.</li>
            {/if}
          {:else}
            {#each rows as r (r.kind + r.path)}
              <li>
                {#if r.kind === "dir"}
                  <button type="button" class="row" style:padding-left="{8 + r.depth * 14}px" aria-expanded={r.open} onclick={() => toggleFolder(r.path)}>
                    <span class="tw">{r.open ? "▾" : "▸"}</span><span class="p dirname">{r.name}</span><span class="cnt">{n(r.count)}</span>
                  </button>
                {:else}
                  <button
                    type="button"
                    class="row"
                    class:current={r.path === file}
                    aria-pressed={r.path === file}
                    style:padding-left="{8 + r.depth * 14}px"
                    title={r.path}
                    onclick={() => (file = r.path)}
                  >
                    <span class="badge l-{LETTER[r.change]}">{LETTER[r.change]}</span><span class="p">{r.name}</span>
                  </button>
                {/if}
              </li>
            {/each}
          {/if}
        </ul>
      </div>

      <div class="col viewer" role="region" aria-label="File">
        {#if file && sides}
          <h3 class="sub vh">
            <span class="fname" title={file}>{file.slice(file.lastIndexOf("/") + 1)}</span>
            <span class="r">
              <ViewSwitch path={file} />
              {#if b.kind === "rev"}
                {@const takeFrom = b.rev}
                <button
                  type="button"
                  class="sm"
                  disabled={busy}
                  title="Make the working copy's {file} what it is in {takeFrom}, staged (Ctrl+Z undoes it)"
                  onclick={() => file && ontake(takeFrom, file)}>Take B's version</button
                >
              {/if}
            </span>
          </h3>
          <div class="view">
            {#if fileError}
              <p class="note bad" role="alert">{fileError}</p>
            {:else if showVisual}
              <FilePreview
                {repoId}
                path={file}
                before={{ label: `A · ${result.mode === "brings" ? "where they split" : sideName(a)}`, at: imageAt(sides.from) }}
                after={{ label: `B · ${sideName(b)}`, at: imageAt(sides.to) }}
                meta={diff?.binaryMeta ?? null}
              />
            {:else if unityDiff && structural}
              <UnityDiffView diff={unityDiff} />
            {:else if diff}
              <DiffView
                {diff}
                split={prefs.split}
                images={isImage(file)
                  ? {
                      before: { label: `A · ${sideName(a)}`, url: imageUrl(repoId, file, imageAt(sides.from), 0) },
                      after: { label: `B · ${sideName(b)}`, url: imageUrl(repoId, file, imageAt(sides.to), 0) },
                    }
                  : null}
              />
            {:else}
              <p class="note"><LoadingBar label="Loading diff…" /></p>
            {/if}
          </div>
        {:else}
          <p class="note">Pick a file to see it on both sides.</p>
        {/if}
      </div>
    </div>
  {/if}
</section>

<style>
  .compare {
    display: flex;
    flex-direction: column;
    min-height: 0;
    height: 100%;
    background: var(--bg);
  }
  .bar {
    display: flex;
    align-items: center;
    gap: 8px;
    flex-wrap: wrap;
    padding: 6px 10px;
    border-bottom: 1px solid var(--line);
    background: var(--panel);
  }
  .sp {
    flex: 1;
  }
  .seg {
    display: inline-flex;
    border: 1px solid var(--line);
    border-radius: 4px;
    overflow: hidden;
  }
  .seg button {
    all: unset;
    cursor: pointer;
    padding: 1px 8px;
    font-size: 11.5px;
    color: var(--muted);
    white-space: nowrap;
  }
  .seg button.on {
    background: var(--sel);
    color: var(--fg);
  }
  .pred {
    font-size: 11.5px;
    color: var(--add-ink);
  }
  .pred.bad {
    color: var(--warn-ink);
  }
  .k {
    font: 600 9.5px var(--mono);
    border: 1px solid var(--line);
    border-radius: 3px;
    padding: 0 3px;
    color: var(--muted);
  }
  .info {
    display: flex;
    gap: 10px;
    align-items: center;
    margin: 8px 10px 0;
    padding: 6px 10px;
    border-radius: 5px;
    background: var(--hover);
    color: var(--muted);
    font-size: 12px;
  }
  .info b {
    color: var(--fg);
  }
  .info.warn {
    background: var(--warn);
    color: var(--warn-ink);
  }
  .note {
    padding: 10px 12px;
    color: var(--muted);
  }
  .bad {
    color: var(--bad-ink);
  }
  .body {
    flex: 1;
    min-height: 0;
    display: grid;
    grid-template-columns: minmax(220px, 290px) minmax(260px, 1fr) minmax(320px, 1.3fr);
  }
  .body.loading {
    opacity: 0.6;
  }
  .col {
    min-height: 0;
    display: flex;
    flex-direction: column;
    overflow: hidden;
  }
  .col + .col {
    border-left: 1px solid var(--line);
  }
  .commits {
    overflow-y: auto;
  }
  .sub {
    display: flex;
    gap: 8px;
    align-items: center;
    margin: 0;
    padding: 8px 10px 4px;
    font: 600 10px var(--mono);
    text-transform: uppercase;
    letter-spacing: 0.05em;
    color: var(--muted);
  }
  .sub .r {
    margin-left: auto;
    text-transform: none;
    letter-spacing: 0;
    display: inline-flex;
    gap: 6px;
    align-items: center;
  }
  .vh {
    text-transform: none;
    letter-spacing: 0;
    font: 600 12px var(--body);
    color: var(--fg);
  }
  .fname {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  ul {
    list-style: none;
    margin: 0;
    padding: 0;
  }
  .commits li {
    display: grid;
    grid-template-columns: 16px minmax(0, 1fr) auto;
    gap: 6px;
    align-items: center;
    padding: 1px 10px;
    font-size: 12px;
  }
  .commits .s {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .who {
    font: 10.5px var(--mono);
    color: var(--muted);
  }
  .none,
  .more {
    margin: 0;
    padding: 2px 10px 6px;
    color: var(--muted);
    font-size: 11.5px;
  }
  .ab {
    display: inline-grid;
    place-items: center;
    width: 13px;
    height: 13px;
    border-radius: 3px;
    font: 800 8px var(--mono);
    color: #10141b;
  }
  .ab.a {
    background: var(--yours);
  }
  .ab.b {
    background: var(--theirs);
  }
  .kinds {
    display: flex;
    gap: 5px;
    flex-wrap: wrap;
    padding: 0 10px 6px;
  }
  .chip {
    font: 600 10px var(--mono);
    padding: 1px 6px;
    border-radius: 9px;
    background: var(--hover);
    color: var(--muted);
  }
  .tree {
    flex: 1;
    overflow-y: auto;
  }
  .row {
    all: unset;
    box-sizing: border-box;
    display: flex;
    gap: 6px;
    align-items: center;
    width: 100%;
    height: var(--row-h-sm, 20px);
    padding: 0 10px 0 8px;
    cursor: pointer;
    font: 12px var(--mono);
  }
  .row:hover {
    background: var(--hover);
  }
  .row.current {
    background: var(--sel);
  }
  .p {
    flex: 1;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .dir {
    color: var(--muted);
  }
  .badge {
    all: unset;
    font: 600 10.5px var(--mono);
    width: 14px;
    text-align: center;
    flex: none;
    color: var(--l4);
  }
  .l-A {
    color: var(--l2);
  }
  .l-D {
    color: var(--bad-ink);
  }
  .tw {
    width: 10px;
    color: var(--muted);
  }
  .cnt {
    font: 11px var(--mono);
    color: var(--muted);
  }
  .view {
    flex: 1;
    min-height: 0;
    display: flex;
    flex-direction: column;
  }
  .view > :global(*) {
    flex: 1;
    min-height: 0;
  }
</style>
