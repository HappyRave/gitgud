<script lang="ts">
  import type { Snippet } from "svelte";
  import { SvelteSet } from "svelte/reactivity";
  import type { MergeSide, MergeState } from "$lib/api";
  import { groupByKind } from "$lib/merge/resolve";
  import ConflictList from "./ConflictList.svelte";
  import MergeSummary from "./MergeSummary.svelte";
  import SidePick from "./SidePick.svelte";
  import TextMerge from "./TextMerge.svelte";

  let {
    repoId,
    merge,
    revision,
    branch,
    how,
    message = $bindable(),
    busy = false,
    onresolvewith,
    onresolveside,
    oncommit,
    children,
    whenResolved,
  }: {
    /** Shown instead of the merge summary once every file is resolved (a rebase's Continue). */
    whenResolved?: Snippet;
    repoId: number;
    merge: MergeState;
    revision: number;
    /** The current branch: "yours". */
    branch: string;
    how: ReadonlyMap<string, string>;
    message: string;
    busy?: boolean;
    /** Resolve to whether the write worked. */
    onresolvewith: (path: string, content: string, how: string) => Promise<boolean>;
    onresolveside: (path: string, side: MergeSide) => Promise<boolean>;
    oncommit: () => void;
    /** Shown instead of the resolver (the op log, Ctrl+3). */
    children?: Snippet;
  } = $props();

  /** Files in list order (grouped by kind). */
  const ordered = $derived(groupByKind(merge.conflicts).flatMap((g) => g.conflicts));
  const left = $derived(ordered.filter((c) => !c.resolved));

  let chosen = $state<string | null>(null);
  const selected = $derived(ordered.find((c) => c.path === chosen) ?? left[0] ?? null);
  /** Text files git couldn't merge as text after all. */
  const sidePickOnly = new SvelteSet<string>();

  const theirsLabel = $derived(merge.theirs ?? "MERGE_HEAD");

  /** After a pick, the next file still waiting (after this one, then from the top). */
  function advance(from: string) {
    const at = ordered.findIndex((c) => c.path === from);
    const next = [...ordered.slice(at + 1), ...ordered.slice(0, at)].find((c) => !c.resolved);
    chosen = next?.path ?? null;
  }

  async function withText(path: string, content: string, label: string) {
    if (await onresolvewith(path, content, label)) {
      advance(path);
    }
  }

  async function withSide(path: string, side: MergeSide) {
    if (await onresolveside(path, side)) {
      advance(path);
    }
  }
</script>

<div class="cv">
  <div class="contents" data-pane="conflicts">
    <ConflictList conflicts={ordered} selected={left.length ? (selected?.path ?? null) : null} {how} onselect={(p) => (chosen = p)} />
  </div>
  <div class="res" data-pane="resolver">
    {#if children}
      {@render children()}
    {:else if left.length === 0 && whenResolved}
      {@render whenResolved()}
    {:else if left.length === 0}
      <MergeSummary conflicts={ordered} {how} bind:message {busy} {oncommit} />
    {:else if selected?.resolved}
      <div class="ph"><span class="name">{selected.path}</span><span class="ct">· resolved</span></div>
      <div class="empty">
        <b>Resolved: {how.get(selected.path) ?? "staged"}</b>
        <span>Ctrl+Z undoes the last pick and brings its conflict back.</span>
        <span><button onclick={() => advance(selected.path)}>Next conflict</button></span>
      </div>
    {:else if selected}
      {#key selected.path}
        {#if selected.kind === "binary" || sidePickOnly.has(selected.path)}
          <SidePick
            {repoId}
            path={selected.path}
            {revision}
            yoursLabel={branch}
            {theirsLabel}
            {busy}
            onpick={(side) => withSide(selected.path, side)}
          />
        {:else}
          <TextMerge
            {repoId}
            path={selected.path}
            {revision}
            unity={selected.kind === "unityYaml"}
            yoursLabel={branch}
            {theirsLabel}
            {busy}
            onsave={(content, label) => withText(selected.path, content, label)}
            onunmergeable={() => sidePickOnly.add(selected.path)}
          />
        {/if}
      {/key}
    {/if}
  </div>
</div>

<style>
  .cv {
    display: grid;
    grid-template-columns: 17rem minmax(0, 1fr);
    min-height: 0;
    height: 100%;
    border-left: 1px solid var(--line);
  }
  .contents {
    display: contents;
  }
  .res {
    display: flex;
    flex-direction: column;
    min-width: 0;
    min-height: 0;
    background: var(--panel);
  }
  .res > :global(*) {
    flex: 1;
    min-height: 0;
  }
  .res > .ph {
    flex: none;
  }
  .name {
    text-transform: none;
    letter-spacing: 0;
    color: var(--fg);
  }
</style>
