<script lang="ts">
  import { untrack } from "svelte";
  import { type BinaryMeta, type BinarySide, type MaterialView, materialUsers, materialView } from "$lib/api";
  import { imageUrl } from "$lib/images";
  import type { ModelSide, Opened } from "$lib/model/open";
  import { toVersion } from "$lib/model/source";
  import { setViewerChoice, viewerChoice } from "$lib/prefs.svelte";
  import { diffMaterial, type MaterialShape, materialPill, SHAPE_LABEL, SHAPES } from "$lib/viewers/material";
  import LoadingBar from "./LoadingBar.svelte";
  import ModelViewer from "./ModelViewer.svelte";

  let {
    repoId,
    path,
    before = null,
    after,
    bust = 0,
    meta = null,
    onfetch,
  }: {
    repoId: number;
    path: string;
    before?: ModelSide | null;
    after: ModelSide;
    bust?: number;
    meta?: BinaryMeta | null;
    onfetch?: (side: "before" | "after", meta: BinarySide) => Promise<void>;
  } = $props();

  let views = $state<{ before: MaterialView | null; after: MaterialView | null; error: string | null } | null>(null);
  let users = $state<string[]>([]);
  let user = $state<string | null>(null);
  let shape = $state<MaterialShape>(untrack(() => viewerChoice(path, "shape", SHAPES)));
  let showAll = $state(false);
  let seq = 0;

  $effect(() => {
    const token = ++seq;
    void [repoId, path, bust, JSON.stringify(before?.at), JSON.stringify(after.at)];
    views = null;
    users = [];
    showAll = false;
    const read = (side: ModelSide | null) =>
      side ? materialView(repoId, path, toVersion(side.at)).then((v) => v, () => null) : Promise.resolve(null);
    Promise.all([
      read(before),
      materialView(repoId, path, toVersion(after.at)).then(
        (v) => ({ view: v, error: null }),
        (e) => ({ view: null, error: String(e instanceof Error ? e.message : e) }),
      ),
      materialUsers(repoId, path, toVersion(after.at)).catch(() => [] as string[]),
    ]).then(([b, a, u]) => {
      if (token !== seq) {
        return;
      }
      views = { before: b, after: a.view, error: a.error };
      users = u;
      user = u[0] ?? null;
    });
  });

  const diff = $derived(views ? diffMaterial(views.before, views.after) : null);
  const shown = $derived(shape === "mesh" && !user ? "sphere" : shape);
  const rows = $derived(diff ? (showAll || !views?.before ? diff.rows : diff.rows.filter((r) => r.changed)) : []);
  const hidden = $derived(diff && views?.before ? diff.rows.length - diff.changed : 0);

  function pick(s: MaterialShape) {
    shape = s;
    setViewerChoice(path, "shape", s);
  }

  async function opener(side: ModelSide): Promise<Opened> {
    const view = side === before ? views?.before : views?.after;
    if (!view) {
      return { model: null, error: `${path} is not in that version` };
    }
    const { primitiveModel } = await import("$lib/model/primitive");
    const prim = shown === "mesh" ? "sphere" : shown;
    return { model: primitiveModel(prim, view, (p) => imageUrl(repoId, p, side.at, bust)), error: null };
  }

  const swatch = (c: [number, number, number, number] | null) =>
    c ? `rgb(${c
      .slice(0, 3)
      .map((v) => Math.round(Math.min(1, Math.max(0, v)) * 255))
      .join(" ")})` : null;
</script>

{#snippet shapes()}
  <div class="seg" role="radiogroup" aria-label="Preview shape">
    {#each SHAPES as s (s)}
      <button
        type="button"
        class="tbtn"
        class:on={shown === s}
        role="radio"
        aria-checked={shown === s}
        disabled={s === "mesh" && users.length === 0}
        title={s === "mesh" && users.length === 0 ? "No prefab or model in the repo uses this material" : undefined}
        onclick={() => pick(s)}>{SHAPE_LABEL[s]}</button
      >
    {/each}
  </div>
  {#if shown === "mesh" && users.length > 1}
    <select class="users" aria-label="Mesh that uses this material" bind:value={user}>
      {#each users as u (u)}<option value={u}>{u.slice(u.lastIndexOf("/") + 1)}</option>{/each}
    </select>
  {/if}
{/snippet}

<div class="mat">
  {#if !views}
    <p class="state"><LoadingBar label="Reading the material…" /></p>
  {:else if views.error || !views.after}
    <p class="state err" role="alert">{views.error}</p>
  {:else}
    <div class="stage">
      {#if shown === "mesh" && user}
        {#key user}
          <ModelViewer {repoId} path={user} {before} {after} {bust} chrome={shapes} />
        {/key}
      {:else}
        {#key shown}
          <ModelViewer {repoId} {path} {before} {after} {bust} {meta} {onfetch} {opener} chrome={shapes} />
        {/key}
      {/if}
    </div>
    <div class="table">
      <table>
        <thead>
          <tr>
            <th>Property</th>
            {#if before}<th>{before.label}</th>{/if}
            <th>{after.label}</th>
          </tr>
        </thead>
        <tbody>
          {#each rows as r (r.key)}
            <tr class:stale={r.after?.stale ?? r.before?.stale}>
              <td title={r.key}>{r.label}{#if r.after?.stale ?? r.before?.stale}<span class="tag">stale</span>{/if}</td>
              {#if before}
                <td>
                  {#if r.before?.color}<span class="sw" style:background={swatch(r.before.color)}></span>{/if}{r.before?.value ?? "—"}
                </td>
              {/if}
              <td class:mod={r.changed}>
                {#if r.after?.color}<span class="sw" style:background={swatch(r.after.color)}></span>{/if}{r.after?.value ?? "—"}
              </td>
            </tr>
          {/each}
        </tbody>
      </table>
    </div>
    <div class="cap">
      <span class="pill">{materialPill(views.after)}</span>
      {#if views.before && diff}
        <b>{diff.changed} changed</b>
        {#each diff.keywordsAdded as k (k)}· keyword <span class="d-add">+{k}</span>{/each}
        {#each diff.keywordsRemoved as k (k)}· keyword <span class="d-del">−{k}</span>{/each}
        {#if hidden > 0}
          · {showAll ? "all shown" : `${hidden} unchanged hidden`}
          <button type="button" class="tbtn solo" onclick={() => (showAll = !showAll)}>{showAll ? "Changes only" : "Show all"}</button>
        {/if}
      {:else}
        <b>{views.after.properties.length} properties</b>
        {#if views.after.keywords.length}· keywords {views.after.keywords.join(" ")}{/if}
      {/if}
    </div>
  {/if}
</div>

<style>
  .mat {
    display: grid;
    grid-template-rows: minmax(300px, 1fr) auto auto;
    height: 100%;
    min-height: 0;
    background: var(--panel);
  }
  .stage {
    min-height: 0;
    display: flex;
  }
  .stage > :global(*) {
    flex: 1;
    min-height: 0;
  }
  .state {
    margin: 12px 10px;
    font-size: 11.5px;
  }
  .err {
    color: var(--bad-ink);
  }
  .table {
    max-height: 40vh;
    overflow: auto;
    border-top: 1px solid var(--line);
  }
  table {
    width: 100%;
    border-collapse: collapse;
    font-size: 11.5px;
  }
  th,
  td {
    text-align: left;
    padding: 3px 10px;
    border-bottom: 1px solid var(--line);
    vertical-align: middle;
  }
  th {
    color: var(--muted);
    font-weight: 500;
    position: sticky;
    top: 0;
    background: var(--panel);
  }
  td + td {
    font-family: var(--mono);
    font-size: 11px;
  }
  td.mod {
    background: var(--warn);
    color: var(--warn-ink);
  }
  tr.stale td:first-child {
    color: var(--muted);
  }
  .tag {
    margin-left: 6px;
    font-size: 9.5px;
    padding: 0 5px;
    border-radius: 999px;
    background: var(--hover);
    color: var(--muted);
  }
  .sw {
    display: inline-block;
    width: 10px;
    height: 10px;
    margin-right: 5px;
    border-radius: 2px;
    border: 1px solid var(--line);
    vertical-align: -1px;
  }
  .cap {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 6px;
    padding: 5px 10px;
    border-top: 1px solid var(--line);
    font-size: 11px;
    color: var(--muted);
  }
  .pill {
    font: 10.5px var(--mono);
    padding: 1px 7px;
    border-radius: 999px;
    background: var(--hover);
  }
  .d-add {
    color: var(--add-ink);
  }
  .d-del {
    color: var(--bad-ink);
  }
  .seg {
    display: inline-flex;
    border: 1px solid rgb(255 255 255 / 0.16);
    border-radius: 5px;
    overflow: hidden;
  }
  .tbtn {
    all: unset;
    font-size: 10.5px;
    padding: 1px 7px;
    color: #c9d0da;
    border-right: 1px solid rgb(255 255 255 / 0.16);
    cursor: default;
    white-space: nowrap;
  }
  .tbtn:last-child {
    border-right: 0;
  }
  .tbtn.on {
    background: rgb(255 255 255 / 0.18);
    color: #fff;
    font-weight: 600;
  }
  .tbtn:hover:not(:disabled) {
    background: rgb(255 255 255 / 0.1);
    color: #fff;
  }
  .tbtn:disabled {
    opacity: 0.4;
  }
  .tbtn:focus-visible {
    outline: 2px solid var(--l0);
  }
  .cap .tbtn.solo {
    color: var(--muted);
    border: 1px solid var(--line);
    border-radius: 5px;
  }
  .cap .tbtn.solo:hover {
    background: var(--hover);
    color: var(--fg);
  }
  .users {
    font-size: 10.5px;
    max-width: 180px;
  }
</style>
