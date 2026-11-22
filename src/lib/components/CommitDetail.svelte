<script lang="ts">
  import {
    commitDetail,
    commitFileDiff,
    commitUnityDiff,
    type Change,
    type CommitDetail,
    type FileDiff,
    type UnityYamlDiff,
  } from "$lib/api";
  import { imageUrl, isImage } from "$lib/images";
  import { prefs, toggleIgnoreWhitespace } from "$lib/prefs.svelte";
  import { isUnityYaml } from "$lib/unity";
  import DiffView from "./DiffView.svelte";
  import UnityDiffView from "./UnityDiffView.svelte";

  let {
    repoId,
    commit,
    onhistory,
    onblame,
    onclose,
  }: {
    repoId: number;
    commit: string;
    onhistory?: (path: string) => void;
    onblame?: (path: string, rev: string) => void;
    onclose: () => void;
  } = $props();

  const LETTER: Record<Change, string> = { added: "A", modified: "M", deleted: "D", typeChange: "T" };

  let detail = $state<CommitDetail | null>(null);
  let file = $state<string | null>(null);
  let diff = $state<FileDiff | null>(null);
  let unityDiff = $state<UnityYamlDiff | null>(null);
  let preferStructural = $state(true);
  let error = $state<string | null>(null);

  $effect(() => {
    const id = commit;
    detail = null;
    file = null;
    diff = null;
    error = null;
    commitDetail(repoId, id).then(
      (d) => {
        if (commit === id) {
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
    const id = commit;
    file = path;
    diff = null;
    unityDiff = null;
    const structural = isUnityYaml(path) && preferStructural;
    const stale = () => commit !== id || file !== path || prefs.ignoreWhitespace !== ignoreWs || (isUnityYaml(path) && preferStructural) !== structural;
    const loadRaw = () => {
      unityDiff = null;
      commitFileDiff(repoId, id, path, ignoreWs).then(
        (d) => {
          if (!stale()) {
            diff = d;
          }
        },
        (err) => {
          if (!stale()) {
            error = String(err);
          }
        },
      );
    };
    if (structural) {
      commitUnityDiff(repoId, id, path).then(
        (u) => {
          if (!stale()) {
            unityDiff = u;
          }
        },
        () => {
          if (!stale()) {
            loadRaw();
          }
        },
      );
    } else {
      loadRaw();
    }
  }

  $effect(() => {
    const ws = prefs.ignoreWhitespace;
    const _pref = preferStructural;
    void _pref;
    if (detail && file) {
      showFile(file, ws);
    }
  });

  function date(seconds: number) {
    return new Date(seconds * 1000).toLocaleString();
  }
</script>

<section class="detail" aria-label="Commit">
  <div class="ph">
    Commit <span class="ct">· {detail?.shortId ?? commit.slice(0, 7)}</span>
    <span class="r">
      {#if file && isUnityYaml(file)}
        <button
          type="button"
          class="close"
          class:ws-on={preferStructural}
          aria-pressed={preferStructural}
          title="Toggle Unity structural property diff vs raw YAML diff"
          onclick={() => (preferStructural = !preferStructural)}>{preferStructural ? "Structural" : "Raw"}</button
        >
      {/if}
      {#if file && onhistory}
        <button type="button" class="close" title="File history" onclick={() => file && onhistory(file)}>History</button>
      {/if}
      {#if file && onblame}
        <button
          type="button"
          class="close"
          title="Annotate file at this commit"
          onclick={() => file && onblame(file, detail?.id ?? commit)}>Blame</button
        >
      {/if}
      <button
        type="button"
        class="close"
        class:ws-on={prefs.ignoreWhitespace}
        aria-pressed={prefs.ignoreWhitespace}
        title="Ignore whitespace changes"
        onclick={toggleIgnoreWhitespace}>±ws</button
      >
      <button class="close" onclick={onclose} aria-label="Back to working copy" title="Esc in the graph">✕ Close</button>
    </span>
  </div>

  {#if error}
    <p role="alert" class="pad">{error}</p>
  {/if}

  {#if detail}
    <div class="card">
      <div class="cbd">
        <h2>{detail.subject}</h2>
        {#if detail.body}
          <p class="body">{detail.body}</p>
        {/if}
        <dl>
          <dt>Author</dt>
          <dd>{detail.author.name} &lt;{detail.author.email}&gt; · {date(detail.author.time)}</dd>
          <dt>Commit</dt>
          <dd class="mono">{detail.id}</dd>
          <dt>Parents</dt>
          <dd class="mono">{detail.parents.map((p) => p.slice(0, 7)).join(", ") || "none (root commit)"}</dd>
        </dl>
      </div>
    </div>

    <div class="grp">{detail.files.length} {detail.files.length === 1 ? "file" : "files"}</div>
    <ul class="files" aria-label="Changed files">
      {#each detail.files as f (f.path)}
        {@const cut = f.path.lastIndexOf("/") + 1}
        <li>
          <button class="bare" class:current={f.path === file} aria-pressed={f.path === file} title={f.path} onclick={() => showFile(f.path)}>
            <span class="badge l-{LETTER[f.change]}">{LETTER[f.change]}</span><span class="fp"><span class="dir">{f.path.slice(0, cut)}</span>{f.path.slice(cut)}</span>
          </button>
        </li>
      {/each}
    </ul>

    {#if unityDiff && file && isUnityYaml(file) && preferStructural}
      <UnityDiffView diff={unityDiff} />
    {:else if diff}
      <DiffView
        {diff}
        split={prefs.split}
        images={file && isImage(file)
          ? {
              before: { label: `Parent ${detail.parents[0]?.slice(0, 7) ?? "(none)"}`, url: imageUrl(repoId, file, { at: "parent", commit }) },
              after: { label: detail.shortId, url: imageUrl(repoId, file, { at: "commit", commit }) },
            }
          : null}
      />
    {/if}
  {:else if !error}
    <p class="note">Loading…</p>
  {/if}
</section>

<style>
  .detail {
    display: flex;
    flex-direction: column;
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
  .body {
    white-space: pre-wrap;
    margin: 0;
  }
  dl {
    display: grid;
    grid-template-columns: auto 1fr;
    gap: 1px 10px;
    margin: 0;
    font-size: 11.5px;
    color: var(--muted);
  }
  dd {
    margin: 0;
    overflow-wrap: anywhere;
  }
  .mono {
    font-family: var(--mono);
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
  }
  .files button {
    all: unset;
    box-sizing: border-box;
    display: flex;
    align-items: center;
    gap: 6px;
    width: 100%;
    height: 24px;
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
  .files button:focus-visible {
    outline: 2px solid var(--l0);
    outline-offset: -2px;
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
</style>
