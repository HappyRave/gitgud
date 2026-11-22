<script lang="ts">
  import { commitDetail, commitFileDiff, type Change, type CommitDetail, type FileDiff } from "$lib/api";
  import DiffView from "./DiffView.svelte";

  let { repoId, commit, onclose }: { repoId: number; commit: string; onclose: () => void } = $props();

  const LETTER: Record<Change, string> = { added: "A", modified: "M", deleted: "D", typeChange: "T" };

  let detail = $state<CommitDetail | null>(null);
  let file = $state<string | null>(null);
  let diff = $state<FileDiff | null>(null);
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

  function showFile(path: string) {
    const id = commit;
    file = path;
    diff = null;
    commitFileDiff(repoId, id, path).then(
      (d) => {
        if (commit === id && file === path) {
          diff = d;
        }
      },
      (err) => (error = String(err)),
    );
  }

  function date(seconds: number) {
    return new Date(seconds * 1000).toLocaleString();
  }
</script>

<section class="detail" aria-label="Commit">
  <header>
    <button class="close" onclick={onclose} aria-label="Back to working copy">Close</button>
    {#if detail}
      <h2>{detail.subject}</h2>
    {/if}
  </header>

  {#if error}
    <p role="alert">{error}</p>
  {/if}

  {#if detail}
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

    <ul class="files" aria-label="Changed files">
      {#each detail.files as f (f.path)}
        <li>
          <button class:current={f.path === file} aria-pressed={f.path === file} onclick={() => showFile(f.path)}>
            <span class="badge">{LETTER[f.change]}</span>{f.path}
          </button>
        </li>
      {/each}
    </ul>

    {#if diff}
      <DiffView {diff} />
    {/if}
  {:else if !error}
    <p class="note">Loading…</p>
  {/if}
</section>

<style>
  .detail {
    display: flex;
    flex-direction: column;
    gap: 0.5rem;
    min-height: 0;
  }
  header {
    display: flex;
    align-items: baseline;
    gap: 0.5rem;
  }
  h2 {
    font-size: 1rem;
    margin: 0;
  }
  .close {
    order: 2;
    margin-left: auto;
  }
  .body {
    white-space: pre-wrap;
    margin: 0;
    font-size: 0.85rem;
  }
  dl {
    display: grid;
    grid-template-columns: auto 1fr;
    gap: 0.1rem 0.6rem;
    margin: 0;
    font-size: 0.8rem;
    color: #555;
  }
  dd {
    margin: 0;
    overflow-wrap: anywhere;
  }
  .mono {
    font-family: ui-monospace, monospace;
  }
  .files {
    list-style: none;
    margin: 0;
    padding: 0;
    font-family: ui-monospace, monospace;
    font-size: 0.85rem;
  }
  .files button {
    all: unset;
    display: block;
    width: 100%;
    line-height: 22px;
    cursor: pointer;
  }
  .files button:hover {
    background: #f4f4f4;
  }
  .files button.current {
    background: #e3ecfd;
  }
  .badge {
    display: inline-block;
    width: 1.5em;
    color: #888;
  }
  .note {
    color: #888;
  }
</style>
