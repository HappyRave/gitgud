<script lang="ts">
  import { lfsFetchObject, stashDetail, stashFileDiff, type BinarySide, type Change, type CommitDetail, type FileDiff } from "$lib/api";
  import { prefs, toggleIgnoreWhitespace } from "$lib/prefs.svelte";
  import DiffView from "./DiffView.svelte";

  let {
    repoId,
    index,
    busy = false,
    onapply,
    onpop,
    ondrop,
    onclose,
  }: {
    repoId: number;
    index: number;
    busy?: boolean;
    onapply: () => void;
    onpop: () => void;
    ondrop: () => void;
    onclose: () => void;
  } = $props();

  const LETTER: Record<Change, string> = { added: "A", modified: "M", deleted: "D", typeChange: "T" };

  let detail = $state<CommitDetail | null>(null);
  let file = $state<string | null>(null);
  let diff = $state<FileDiff | null>(null);
  let error = $state<string | null>(null);

  $effect(() => {
    const idx = index;
    detail = null;
    file = null;
    diff = null;
    error = null;
    stashDetail(repoId, idx).then(
      (d) => {
        if (index === idx) {
          detail = d;
          if (d.files.length > 0) {
            showFile(d.files[0].path);
          }
        }
      },
      (err) => (error = String(err)),
    );
  });

  function showFile(path: string, ignoreWs = prefs.ignoreWhitespace) {
    const idx = index;
    file = path;
    diff = null;
    stashFileDiff(repoId, idx, path, ignoreWs).then(
      (d) => {
        if (index === idx && file === path && prefs.ignoreWhitespace === ignoreWs) {
          diff = d;
        }
      },
      (err) => (error = String(err)),
    );
  }

  $effect(() => {
    const ws = prefs.ignoreWhitespace;
    if (detail && file) {
      showFile(file, ws);
    }
  });

  async function handleFetchLfs(_which: "before" | "after", meta: BinarySide) {
    if (!meta.oid || !file) {
      return;
    }
    const idx = index;
    const path = file;
    const ws = prefs.ignoreWhitespace;
    await lfsFetchObject(repoId, meta.oid, meta.size, path);
    if (index !== idx || file !== path) {
      return;
    }
    diff = await stashFileDiff(repoId, idx, path, ws);
  }
</script>

<section class="detail" aria-label="Stash">
  <div class="ph">
    Stash <span class="ct">· stash@{`{${index}}`}</span>
    <span class="r">
      <button
        type="button"
        class="close"
        class:on={prefs.ignoreWhitespace}
        aria-pressed={prefs.ignoreWhitespace}
        title="Ignore whitespace changes"
        onclick={toggleIgnoreWhitespace}>±ws</button
      >
      <button class="close" onclick={onclose} aria-label="Back to working copy">✕ Close</button>
    </span>
  </div>

  {#if error}
    <p role="alert" class="pad">{error}</p>
  {/if}

  {#if detail}
    <div class="meta-col">
      <div class="card">
        <div class="cbd">
          <h2>{detail.subject}</h2>
          <div class="ab">Saved {new Date(detail.author.time * 1000).toLocaleString()} · undoable with Ctrl+Z</div>
          <div class="acts">
            <button class="pri" disabled={busy} onclick={onpop}>Pop</button>
            <button disabled={busy} onclick={onapply}>Apply</button>
            <button class="bad" disabled={busy} onclick={ondrop}>Drop</button>
          </div>
        </div>
      </div>

      <div class="grp">{detail.files.length} {detail.files.length === 1 ? "file" : "files"}</div>
      <ul class="files" aria-label="Stashed files">
        {#each detail.files as f (f.path)}
          {@const cut = f.path.lastIndexOf("/") + 1}
          <li>
            <button class="bare" class:current={f.path === file} aria-pressed={f.path === file} title={f.path} onclick={() => showFile(f.path)}>
              <span class="badge l-{LETTER[f.change]}">{LETTER[f.change]}</span><span class="fp"><span class="dir">{f.path.slice(0, cut)}</span>{f.path.slice(cut)}</span>
            </button>
          </li>
        {/each}
      </ul>
    </div>

    <div class="diff-col">
      {#if diff}
        <DiffView {diff} split={prefs.split} onfetchlfs={handleFetchLfs} />
      {/if}
    </div>
  {:else if !error}
    <p class="note">Loading…</p>
  {/if}
</section>

<style>
  .detail {
    display: flex;
    flex-direction: column;
    min-height: 0;
    height: 100%;
  }
  .meta-col {
    display: flex;
    flex-direction: column;
    flex: 0 1 auto;
    min-height: 0;
  }
  .diff-col {
    display: flex;
    flex-direction: column;
    flex: 1;
    min-height: 0;
  }
  .diff-col > :global(*) {
    flex: 1;
    min-height: 0;
  }
  .close {
    height: 20px;
    padding: 0 7px;
    font-size: 11px;
  }
  h2 {
    font: 600 14px var(--display);
    margin: 0;
  }
  .acts {
    display: flex;
    gap: 6px;
  }
  .pad,
  .note {
    padding: 0 10px;
    color: var(--muted);
  }
  .files {
    list-style: none;
    margin: 0;
    padding: 2px 0;
    max-height: 180px;
    overflow-y: auto;
    border-bottom: 1px solid var(--line);
    flex: none;
  }
  .files button {
    all: unset;
    box-sizing: border-box;
    display: flex;
    align-items: center;
    gap: 6px;
    width: 100%;
    height: var(--row-h, 24px);
    padding: 0 10px;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    cursor: default;
  }
  .files button:hover {
    background: var(--hover);
  }
  .files button.current {
    background: var(--sel);
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
  .fp {
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .dir {
    color: var(--muted);
  }
  @media (min-width: 2400px) {
    .detail {
      display: grid;
      grid-template-columns: minmax(320px, 2fr) 3fr;
      grid-template-rows: auto minmax(0, 1fr);
    }
    .ph {
      grid-column: 1 / -1;
    }
    .meta-col {
      grid-area: 2 / 1;
      overflow-y: auto;
    }
    .meta-col .files {
      max-height: none;
      flex: 1;
      border-bottom: 0;
    }
    .diff-col {
      grid-area: 2 / 2;
      border-left: 1px solid var(--line);
    }
  }
</style>
