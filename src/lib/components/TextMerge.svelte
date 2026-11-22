<script lang="ts">
  import { SvelteMap, SvelteSet } from "svelte/reactivity";
  import { tick, untrack } from "svelte";
  import { conflictText, type Chunk, type ConflictText } from "$lib/api";
  import { conflictKey } from "$lib/keys";
  import { conflictIndices, describePicks, foldSame, pickLines, resultText, type Pick } from "$lib/merge/resolve";

  let {
    repoId,
    path,
    revision,
    unity = false,
    yoursLabel,
    theirsLabel,
    busy = false,
    onsave,
    onunmergeable,
  }: {
    repoId: number;
    path: string;
    revision: number;
    /** A Unity YAML file merged as text (no Smart Merge yet). */
    unity?: boolean;
    yoursLabel: string;
    theirsLabel: string;
    busy?: boolean;
    onsave: (content: string, how: string) => void;
    /** The file can't be merged as text (binary, deleted on one side): pick a side instead. */
    onunmergeable: () => void;
  } = $props();

  type ConflictChunk = Extract<Chunk, { kind: "conflict" }>;

  let text = $state<ConflictText | null>(null);
  let error = $state<string | null>(null);
  const picks = new SvelteMap<number, Pick>();
  /** Folded runs the user opened, by chunk index. */
  const unfolded = new SvelteSet<number>();
  /** Position in `conflicts` of the hunk the keys act on. */
  let current = $state(0);
  let editing = $state<{ chunk: number; value: string } | null>(null);
  let body = $state<HTMLElement | null>(null);

  const conflicts = $derived(text ? conflictIndices(text.chunks) : []);
  const result = $derived(text ? resultText(text, picks) : null);
  const picked = $derived(conflicts.filter((i) => picks.has(i)).length);

  /** First result line number of each chunk (unpicked conflicts add none). */
  const starts = $derived.by(() => {
    const out: number[] = [];
    let n = 1;
    for (const [i, chunk] of (text?.chunks ?? []).entries()) {
      out.push(n);
      const pick = picks.get(i);
      n += chunk.kind === "same" ? chunk.lines.length : pick ? pickLines(chunk, pick).length : 0;
    }
    return out;
  });

  $effect(() => {
    const key = `${repoId}:${path}`;
    void revision;
    untrack(() => {
      error = null;
    });
    conflictText(repoId, path).then(
      (t) => {
        if (key !== `${repoId}:${path}`) {
          return;
        }
        if (text?.chunks.length !== t.chunks.length) {
          picks.clear();
          unfolded.clear();
          current = 0;
        }
        text = t;
      },
      (err) => {
        const message = String(err);
        if (message.includes("pick a side")) {
          onunmergeable();
        } else {
          error = message;
        }
      },
    );
  });

  async function focusConflict(pos: number) {
    current = Math.max(0, Math.min(conflicts.length - 1, pos));
    await tick();
    body?.querySelector(`[data-conflict="${current}"]`)?.scrollIntoView?.({ block: "center" });
  }

  /** Picks for the current hunk, then moves on to the next one without a pick. */
  function choose(pick: Pick, pos = current) {
    const chunk = conflicts[pos];
    if (chunk === undefined) {
      return;
    }
    picks.set(chunk, pick);
    const next = conflicts.findIndex((i, p) => p > pos && !picks.has(i));
    if (next >= 0) {
      focusConflict(next);
    } else {
      current = pos;
    }
  }

  function startEdit(pos = current) {
    const chunkIndex = conflicts[pos];
    const chunk = text?.chunks[chunkIndex] as ConflictChunk | undefined;
    if (!chunk) {
      return;
    }
    current = pos;
    const pick = picks.get(chunkIndex);
    const lines = pick ? pickLines(chunk, pick) : [...chunk.yours, ...chunk.theirs];
    editing = { chunk: chunkIndex, value: lines.join("\n") };
    tick().then(() => body?.querySelector<HTMLTextAreaElement>("textarea")?.focus());
  }

  function finishEdit() {
    if (!editing) {
      return;
    }
    const lines = editing.value === "" ? [] : editing.value.split(/\r?\n/);
    const pos = conflicts.indexOf(editing.chunk);
    editing = null;
    choose({ kind: "edited", lines }, pos);
  }

  function onEditKey(event: KeyboardEvent) {
    if (event.key === "Escape") {
      event.preventDefault();
      editing = null;
    } else if (event.key === "Enter" && event.ctrlKey) {
      event.preventDefault();
      finishEdit();
    }
  }

  function save() {
    if (result === null || busy) {
      return;
    }
    onsave(result, describePicks(conflicts.map((i) => picks.get(i)!)));
  }

  function onKeydown(event: KeyboardEvent) {
    const action = conflictKey(event);
    if (!action || !text) {
      return;
    }
    event.preventDefault();
    switch (action) {
      case "yours":
      case "theirs":
      case "both":
        choose({ kind: action });
        break;
      case "edit":
        startEdit();
        break;
      case "next":
        focusConflict(current + 1);
        break;
      case "prev":
        focusConflict(current - 1);
        break;
      case "resolve":
        save();
        break;
    }
  }

  const SOURCE: Record<Pick["kind"], string> = { yours: "yours", theirs: "theirs", both: "both", edited: "edited" };
</script>

<svelte:window onkeydown={onKeydown} />

{#snippet lines(list: string[], first: number | null, tone = "")}
  {#each list as line, i (i)}
    <div class="ln {tone}"><span class="no">{first === null ? "" : first + i}</span><code>{line || " "}</code></div>
  {/each}
{/snippet}

<section class="tm" aria-label="Merge {path}">
  <div class="ph">
    <span class="name">{path}</span><span class="ct">· 3-way text</span>
    <span class="r">F7 next · Shift+F7 previous</span>
  </div>
  {#if unity}
    <div class="flag note">Smart Merge isn't set up yet, so this Unity file merges as plain text.</div>
  {/if}

  {#if error}
    <p role="alert" class="err">{error}</p>
  {:else if text}
    <div class="hk">
      <span class="pos">
        {#if conflicts.length}
          Conflict {current + 1} of {conflicts.length}
        {:else}
          No conflicting hunks: git merged the text, save to mark it resolved
        {/if}
      </span>
      <span class="sp"></span>
      {#if conflicts.length}
        <button class:on={picks.get(conflicts[current])?.kind === "yours"} onclick={() => choose({ kind: "yours" })}
          >Yours <span class="k">Alt+←</span></button
        >
        <button class:on={picks.get(conflicts[current])?.kind === "theirs"} onclick={() => choose({ kind: "theirs" })}
          >Theirs <span class="k">Alt+→</span></button
        >
        <button class:on={picks.get(conflicts[current])?.kind === "both"} onclick={() => choose({ kind: "both" })}
          >Use both, yours first <span class="k">Alt+↓</span></button
        >
        <button onclick={() => startEdit()}>Edit <span class="k">E</span></button>
      {/if}
    </div>

    <div class="heads">
      <span class="yours">Yours · {yoursLabel}</span>
      <span>Result</span>
      <span class="theirs">Theirs · {theirsLabel}</span>
    </div>
    <div class="grid" bind:this={body}>
      {#each text.chunks as chunk, i (i)}
        {#if chunk.kind === "same"}
          {@const fold = unfolded.has(i)
            ? { head: chunk.lines.length, hidden: 0, tail: 0 }
            : foldSame(chunk.lines.length, i > 0, i < text.chunks.length - 1)}
          {#each [null, starts[i], null] as first, col (col)}
            <div class="cell same">
              {@render lines(chunk.lines.slice(0, fold.head), first)}
              {#if fold.hidden}
                <button class="bare fold" onclick={() => unfolded.add(i)}>⋯ {fold.hidden} unchanged lines</button>
                {@render lines(chunk.lines.slice(chunk.lines.length - fold.tail), first === null ? null : first + fold.head + fold.hidden)}
              {/if}
            </div>
          {/each}
        {:else}
          {@const pos = conflicts.indexOf(i)}
          {@const pick = picks.get(i)}
          <div class="cell side yours" class:cur={pos === current} class:dim={pick && pick.kind === "theirs"} data-conflict={pos}>
            <button class="bare arrow" title="Use yours (Alt+←)" aria-label="Use yours" onclick={() => choose({ kind: "yours" }, pos)}>→</button>
            {@render lines(chunk.yours, null)}
            {#if chunk.yours.length === 0}<div class="gone">deleted these lines</div>{/if}
          </div>
          <div class="cell res" class:cur={pos === current} class:open={!pick}>
            {#if editing?.chunk === i}
              <textarea
                aria-label="Edit result"
                rows={Math.max(2, editing.value.split("\n").length)}
                bind:value={editing.value}
                onkeydown={onEditKey}
                onblur={finishEdit}
              ></textarea>
              <span class="ab">Ctrl+↵ done · Esc cancel</span>
            {:else if pick}
              <div class="from {SOURCE[pick.kind]}">
                {@render lines(pickLines(chunk, pick), starts[i])}
                {#if pickLines(chunk, pick).length === 0}<div class="gone">no lines</div>{/if}
              </div>
            {:else}
              <div class="pending">
                Pick a side, or <button class="bare link" onclick={() => choose({ kind: "both" }, pos)}>use both</button>
              </div>
            {/if}
          </div>
          <div class="cell side theirs" class:cur={pos === current} class:dim={pick && pick.kind === "yours"}>
            <button class="bare arrow" title="Use theirs (Alt+→)" aria-label="Use theirs" onclick={() => choose({ kind: "theirs" }, pos)}>←</button>
            {@render lines(chunk.theirs, null)}
            {#if chunk.theirs.length === 0}<div class="gone">deleted these lines</div>{/if}
          </div>
        {/if}
      {/each}
    </div>

    <div class="sum">
      <span class="ab">{picked} of {conflicts.length} picked · saving stages the file · Ctrl+Z undoes it</span>
      <span class="sp"></span>
      <button class="pri" disabled={result === null || busy} onclick={save}>Save and mark resolved <span class="k">Ctrl+↵</span></button>
    </div>
  {:else}
    <p class="ab loading">Loading…</p>
  {/if}
</section>

<style>
  .tm {
    display: flex;
    flex-direction: column;
    min-height: 0;
    height: 100%;
    background: var(--panel);
  }
  .name {
    text-transform: none;
    letter-spacing: 0;
    color: var(--fg);
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .note {
    margin: 6px 10px 0;
    flex: none;
  }
  .err,
  .loading {
    padding: 10px;
    margin: 0;
  }
  .hk {
    display: flex;
    align-items: center;
    gap: 6px;
    min-height: 34px;
    padding: 4px 10px;
    background: var(--hover);
    border-bottom: 1px solid var(--line);
    flex: none;
    flex-wrap: wrap;
  }
  .pos {
    font: 11.5px var(--mono);
    color: var(--muted);
  }
  .sp {
    flex: 1;
  }
  .heads,
  .grid {
    display: grid;
    grid-template-columns: repeat(3, minmax(0, 1fr));
  }
  .heads {
    flex: none;
    border-bottom: 1px solid var(--line);
    background: var(--bg);
    font: 600 11.5px var(--body);
  }
  .heads span {
    padding: 5px 10px;
    border-right: 1px solid var(--line);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .heads .yours {
    color: var(--yours);
  }
  .heads .theirs {
    color: var(--theirs);
  }
  .grid {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
    align-content: start;
    font: 12px/18px var(--mono);
  }
  .cell {
    border-right: 1px solid var(--line);
    min-width: 0;
    position: relative;
  }
  .cell.same {
    color: var(--muted);
  }
  .ln {
    display: grid;
    grid-template-columns: 38px minmax(0, 1fr);
  }
  .no {
    color: var(--muted);
    opacity: 0.7;
    text-align: right;
    padding-right: 8px;
    user-select: none;
  }
  .ln code {
    font: inherit;
    white-space: pre-wrap;
    overflow-wrap: anywhere;
  }
  .side,
  .res {
    border-block: 1px solid var(--line);
    padding: 2px 0;
  }
  .side.yours {
    background: var(--yours-bg);
    color: var(--fg);
  }
  .side.theirs {
    background: var(--theirs-bg);
    color: var(--fg);
  }
  .side.dim {
    opacity: 0.45;
  }
  .cur {
    box-shadow: inset 0 2px 0 var(--l0), inset 0 -2px 0 var(--l0);
  }
  .res.open {
    background: var(--bad);
  }
  .from.yours {
    background: var(--yours-bg);
  }
  .from.theirs {
    background: var(--theirs-bg);
  }
  .from.both {
    background: linear-gradient(90deg, var(--yours-bg), var(--theirs-bg));
  }
  .from.edited {
    background: var(--add);
  }
  /* Sits in the empty line-number gutter of the yours / theirs columns. */
  .arrow {
    all: unset;
    position: absolute;
    top: 2px;
    left: 6px;
    width: 26px;
    height: 18px;
    text-align: center;
    border-radius: 4px;
    font: 600 13px var(--body);
    color: var(--yours);
    background: var(--panel);
    cursor: default;
  }
  .theirs .arrow {
    color: var(--theirs);
  }
  .arrow:hover {
    background: var(--sel);
  }
  .pending {
    padding: 0 10px;
    color: var(--bad-ink);
    font: 11.5px/18px var(--body);
  }
  .link {
    all: unset;
    text-decoration: underline;
    cursor: pointer;
  }
  .gone {
    padding-left: 38px;
    color: var(--muted);
    font: italic 11px var(--body);
  }
  .fold {
    all: unset;
    display: block;
    width: 100%;
    text-align: center;
    color: var(--muted);
    font: 11px var(--body);
    background: var(--bg);
    border-block: 1px dashed var(--line);
    cursor: default;
  }
  .fold:hover {
    color: var(--fg);
  }
  textarea {
    display: block;
    width: calc(100% - 8px);
    margin: 2px 4px;
    font: 12px/18px var(--mono);
    resize: vertical;
    background: var(--panel);
  }
  .res .ab {
    padding: 0 6px;
  }
  .sum {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 8px 10px;
    border-top: 1px solid var(--line);
    flex: none;
  }
</style>
