<script lang="ts">
  import type { Snippet } from "svelte";
  import { SvelteSet } from "svelte/reactivity";
  import type { MergeSide, MergeState, UnityInfo } from "$lib/api";
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
    unityInfo = null,
    onresolvewith,
    onresolveside,
    oncommit,
    onsetupSmartMerge,
    onremergeUnity,
    onconfirmVerify,
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
    unityInfo?: UnityInfo | null;
    /** Resolve to whether the write worked. */
    onresolvewith: (path: string, content: string, how: string) => Promise<boolean>;
    onresolveside: (path: string, side: MergeSide) => Promise<boolean>;
    oncommit: () => void;
    onsetupSmartMerge?: () => void;
    onremergeUnity?: () => void;
    onconfirmVerify?: (path: string, fileId: string | null, property: string | null, confirmed: boolean) => void;
    /** Shown instead of the resolver (the op log, Ctrl+3). */
    children?: Snippet;
  } = $props();

  /** Files in list order (grouped by kind). */
  const ordered = $derived(groupByKind(merge.conflicts).flatMap((g) => g.conflicts));
  const left = $derived(ordered.filter((c) => !c.resolved));
  const unityVerify = $derived(merge.unityVerify ?? []);
  const unconfirmedCount = $derived(
    unityVerify.reduce((acc, f) => acc + f.verify.filter((v) => !v.confirmed).length, 0),
  );
  const unresolvedUnityCount = $derived(
    ordered.filter((c) => !c.resolved && c.kind === "unityYaml").length,
  );

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
    {:else}
      {#if unityInfo?.isUnity && unresolvedUnityCount > 0}
        {#if !unityInfo.smartMergeConfigured && onsetupSmartMerge}
          <section class="smart-card" aria-label="Unity Smart Merge setup">
            <div class="sc-body">
              <strong>Unity Smart Merge is not configured</strong>
              <span>
                Configures <code>UnityYAMLMerge</code> in local <code>.git/config</code> and
                <code>.git/info/attributes</code> only — leaves tracked <code>.gitattributes</code> untouched.
              </span>
            </div>
            <button type="button" class="pri" disabled={busy} onclick={onsetupSmartMerge}>
              Set up Smart Merge &amp; re-merge
            </button>
          </section>
        {:else if unityInfo.smartMergeConfigured && onremergeUnity}
          <section class="smart-card configured" aria-label="Unity Smart Merge">
            <span>{unresolvedUnityCount} Unity YAML conflict(s) can be merged semantically with UnityYAMLMerge.</span>
            <button type="button" disabled={busy} onclick={onremergeUnity}>Re-merge Unity YAML</button>
          </section>
        {/if}
      {/if}

      {#if left.length === 0 && unityVerify.length > 0}
        <section class="verify-panel" aria-label="Smart Merge verification">
          {#each unityVerify as file (file.path)}
            <div class="vfile">
              <div class="vhead">
                <strong>{file.path}</strong>
                {#if file.verify.some((v) => !v.confirmed) && onconfirmVerify}
                  <button
                    type="button"
                    disabled={busy}
                    onclick={() => onconfirmVerify(file.path, null, null, true)}>Confirm all</button
                  >
                {/if}
              </div>
              {#if file.verify.length > 0}
                <ul class="vlist">
                  {#each file.verify as item (item.fileId + item.property)}
                    <li>
                      <label class="vrow">
                        <input
                          type="checkbox"
                          checked={item.confirmed}
                          disabled={busy || !onconfirmVerify}
                          onchange={(e) =>
                            onconfirmVerify?.(file.path, item.fileId, item.property, e.currentTarget.checked)}
                        />
                        <span class="vmeta">
                          <b>{item.objectName}</b> · <code>{item.property}</code>
                        </span>
                        <span class="vvals ab">
                          yours={item.ours ?? "∅"} · theirs={item.theirs ?? "∅"} → merged={item.merged ?? "∅"}
                        </span>
                      </label>
                    </li>
                  {/each}
                </ul>
              {/if}
              {#if file.autoMerged.length > 0}
                <details class="auto-merged">
                  <summary>Auto-merged cleanly ({file.autoMerged.length} properties)</summary>
                  <ul>
                    {#each file.autoMerged as am (am.fileId + am.property)}
                      <li>
                        <b>{am.objectName}</b> · <code>{am.property}</code> ({am.side}): <code>{am.value ?? "∅"}</code>
                      </li>
                    {/each}
                  </ul>
                </details>
              {/if}
            </div>
          {/each}
        </section>
      {/if}

      {#if left.length === 0 && whenResolved}
        {@render whenResolved()}
      {:else if left.length === 0}
        <MergeSummary conflicts={ordered} {how} {unconfirmedCount} bind:message {busy} {oncommit} />
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
    display: flex;
    flex-direction: column;
    min-height: 0;
    border-right: 1px solid var(--line);
  }
  .res {
    display: flex;
    flex-direction: column;
    min-width: 0;
    min-height: 0;
  }
  .smart-card {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 8px 12px;
    background: var(--warn);
    color: var(--warn-ink);
    border-bottom: 1px solid var(--line);
    font-size: 12px;
  }
  .smart-card.configured {
    background: var(--sel);
    color: var(--fg);
  }
  .sc-body {
    flex: 1;
    display: grid;
    gap: 2px;
  }
  .verify-panel {
    padding: 10px 12px;
    border-bottom: 1px solid var(--line);
    background: var(--bg);
    display: grid;
    gap: 8px;
  }
  .vfile {
    background: var(--panel);
    border: 1px solid var(--line);
    border-radius: 6px;
    padding: 8px 10px;
  }
  .vhead {
    display: flex;
    align-items: center;
    justify-content: space-between;
    margin-bottom: 6px;
  }
  .vlist {
    list-style: none;
    margin: 0;
    padding: 0;
    display: grid;
    gap: 4px;
  }
  .vrow {
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: 12px;
  }
  .auto-merged {
    margin-top: 6px;
    font-size: 11.5px;
    color: var(--muted);
  }
  .auto-merged ul {
    margin: 4px 0 0;
    padding-left: 18px;
  }
</style>
