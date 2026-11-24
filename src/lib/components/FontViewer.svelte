<script lang="ts">
  import { onDestroy, untrack } from "svelte";
  import { type BinaryMeta, type BinarySide, type FontInfo, fontInfo } from "$lib/api";
  import type { ModelSide } from "$lib/model/open";
  import { toVersion } from "$lib/model/source";
  import { setViewerChoice, viewerChoice } from "$lib/prefs.svelte";
  import { ALPHABET, diffGlyphs, fontPill, type Glyph, metricRows, SAMPLE } from "$lib/viewers/font";
  import { fileUrl } from "$lib/viewers/route";
  import LfsBinaryCard from "./LfsBinaryCard.svelte";
  import LoadingBar from "./LoadingBar.svelte";

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

  type Loaded = { info: FontInfo; family: string; face: FontFace } | { error: string };
  const TABS = ["sample", "glyphs", "metrics"] as const;
  const TAB_LABEL = { sample: "Sample", glyphs: "All glyphs", metrics: "Metrics" };
  /** Cells drawn before "Show all": a CJK font maps tens of thousands. */
  const GRID_CAP = 3000;
  /** Changed glyphs shown under the sample. */
  const SAMPLE_CAP = 120;

  let sides = $state<{ before: Loaded | null; after: Loaded | null } | null>(null);
  let tab = $state(untrack(() => viewerChoice(path, "tab", TABS)));
  let sample = $state(SAMPLE);
  let expanded = $state(false);
  let showAll = $state(false);
  let axes = $state<Record<string, number>>({});
  let seq = 0;

  async function load(side: ModelSide, n: number): Promise<Loaded> {
    try {
      const info = await fontInfo(repoId, path, toVersion(side.at));
      const family = `gitgud-font-${n}-${seq}`;
      const face = new FontFace(family, `url("${fileUrl(repoId, path, side.at, bust)}")`);
      await face.load();
      document.fonts.add(face);
      return { info, family, face };
    } catch (err) {
      return { error: err instanceof Error ? err.message : String(err) };
    }
  }

  function release() {
    for (const s of [sides?.before, sides?.after]) {
      if (s && "face" in s) {
        document.fonts.delete(s.face);
      }
    }
  }

  $effect(() => {
    const token = ++seq;
    void [repoId, path, bust, JSON.stringify(before?.at), JSON.stringify(after.at)];
    untrack(release);
    sides = null;
    showAll = false;
    Promise.all([before ? load(before, 0) : Promise.resolve(null), load(after, 1)]).then(([b, a]) => {
      if (token !== seq) {
        for (const s of [b, a]) {
          if (s && "face" in s) {
            document.fonts.delete(s.face);
          }
        }
        return;
      }
      sides = { before: b, after: a };
      axes = Object.fromEntries(((a && "info" in a ? a.info : null)?.axes ?? []).map((x) => [x.tag, x.default]));
    });
  });

  onDestroy(release);

  const infoOf = (s: Loaded | null | undefined) => (s && "info" in s ? s.info : null);
  const bInfo = $derived(infoOf(sides?.before));
  const aInfo = $derived(infoOf(sides?.after));
  const diff = $derived(diffGlyphs(bInfo, aInfo));
  const compare = $derived(bInfo !== null && aInfo !== null);
  const failed = $derived(sides ? (sides.after && "error" in sides.after ? sides.after.error : null) : null);
  const variation = $derived(
    Object.entries(axes)
      .map(([t, v]) => `"${t}" ${v}`)
      .join(", "),
  );
  const columns = $derived(
    sides
      ? [
          ...(before && sides.before ? [{ side: before, loaded: sides.before, key: "before" as const }] : []),
          { side: after, loaded: sides.after!, key: "after" as const },
        ]
      : [],
  );
  const changedGlyphs = $derived(diff.glyphs.filter((g) => g.state !== "same").slice(0, SAMPLE_CAP));

  function gridFor(key: "before" | "after"): Glyph[] {
    const hide = key === "before" ? "added" : "removed";
    return diff.glyphs.filter((g) => g.state !== hide);
  }

  function pick(t: (typeof TABS)[number]) {
    tab = t;
    setViewerChoice(path, "tab", t);
  }

  const fmt = (n: number) => n.toLocaleString("en-US");
  const name = (cp: number) => `U+${cp.toString(16).toUpperCase().padStart(4, "0")}`;
  const STATE_WORD = { same: "", added: "added", removed: "removed", changed: "redrawn" };
</script>

<div class="font" class:expanded>
  <div class="bar">
    {#if aInfo ?? bInfo}<span class="pill">{fontPill(path, bInfo, aInfo)}</span>{/if}
    <button type="button" class="tbtn solo" onclick={() => (expanded = !expanded)} aria-pressed={expanded}
      >{expanded ? "Close" : "Expand"}</button
    >
  </div>

  <div class="vp">
    {#if !sides}
      <p class="state"><LoadingBar label="Reading the font…" /></p>
    {:else if failed}
      {#if meta?.after?.lfs}
        <div class="state"><LfsBinaryCard {path} {meta} {onfetch} /></div>
      {:else}
        <p class="state err" role="alert">{failed}</p>
      {/if}
    {:else}
      <div class="chrome">
        <div class="seg" role="radiogroup" aria-label="Font view">
          {#each TABS as t (t)}
            <button type="button" class="tbtn" class:on={tab === t} role="radio" aria-checked={tab === t} onclick={() => pick(t)}
              >{TAB_LABEL[t]}</button
            >
          {/each}
        </div>
      </div>

      {#if tab === "metrics"}
        <table class="metrics">
          <thead>
            <tr><th></th>{#each columns as c (c.key)}<th>{c.side.label}</th>{/each}</tr>
          </thead>
          <tbody>
            {#each metricRows(bInfo, aInfo) as row (row.label)}
              <tr class:changed={row.changed}>
                <th>{row.label}</th>
                {#if before}<td>{row.before ?? "—"}</td>{/if}
                <td>{row.after ?? "—"}</td>
              </tr>
            {/each}
          </tbody>
        </table>
      {:else}
        {#if tab === "sample"}
          <label class="sample-input">
            <span>Sample</span>
            <input type="text" bind:value={sample} aria-label="Sample text" spellcheck="false" />
          </label>
        {/if}
        <div class="split" class:single={columns.length === 1}>
          {#each columns as c (c.key)}
            <section class="col" aria-label={c.side.label}>
              <span class="lbl">{c.side.label}</span>
              {#if "error" in c.loaded}
                <p class="err" role="alert">{c.loaded.error}</p>
              {:else}
                {@const style = `font-family: "${c.loaded.family}", var(--mono); font-variation-settings: ${variation || "normal"}`}
                {#if tab === "sample"}
                  <div class="big" {style}>{sample}</div>
                  <div class="small" {style}>{ALPHABET}</div>
                  {#if compare && changedGlyphs.length}
                    <div class="glyphs" {style}>
                      {#each changedGlyphs as g (g.cp)}
                        <span class={g.state} title="{name(g.cp)} {STATE_WORD[g.state]}"
                          >{(c.key === "before" ? g.state !== "added" : g.state !== "removed") ? String.fromCodePoint(g.cp) : ""}</span
                        >
                      {/each}
                    </div>
                  {/if}
                {:else}
                  {@const grid = gridFor(c.key)}
                  <div class="glyphs" {style}>
                    {#each showAll ? grid : grid.slice(0, GRID_CAP) as g (g.cp)}
                      <span class={g.state} title="{name(g.cp)} {STATE_WORD[g.state]}">{String.fromCodePoint(g.cp)}</span>
                    {/each}
                  </div>
                  {#if !showAll && grid.length > GRID_CAP}
                    <button type="button" class="tbtn solo more" onclick={() => (showAll = true)}>Show all {fmt(grid.length)}</button>
                  {/if}
                {/if}
              {/if}
            </section>
          {/each}
        </div>
      {/if}
    {/if}
  </div>

  {#if aInfo}
    <div class="cap">
      {#if compare && bInfo}
        <b>{fmt(bInfo.glyphCount)} → {fmt(aInfo.glyphCount)} glyphs</b> ·
        <span class="d-add">+{fmt(diff.added)}</span> <span class="d-del">−{fmt(diff.removed)}</span> ·
        <span class="d-mod">{fmt(diff.changed)} outlines changed</span>
      {:else}
        <b>{fmt(aInfo.glyphCount)} glyphs</b> · {fmt(diff.glyphs.length)} characters
      {/if}
      {#if aInfo.faces > 1}· collection of {aInfo.faces} faces, the first shown{/if}
      {#each aInfo.axes as x (x.tag)}
        <label class="axis"
          >{x.name ?? x.tag}
          <input type="range" min={x.min} max={x.max} step="any" bind:value={axes[x.tag]} aria-label="{x.name ?? x.tag} axis" />
          <span>{Math.round(axes[x.tag] ?? x.default)}</span></label
        >
      {/each}
    </div>
  {/if}
</div>

<style>
  .font {
    display: grid;
    grid-template-rows: auto minmax(0, 1fr) auto;
    min-height: 420px;
    height: 100%;
    background: var(--panel);
  }
  .font.expanded {
    position: fixed;
    inset: 0;
    z-index: 50;
  }
  .bar {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 6px 10px;
    border-bottom: 1px solid var(--line);
  }
  .bar .solo {
    margin-left: auto;
  }
  .pill {
    font: 10.5px var(--mono);
    padding: 1px 7px;
    border-radius: 999px;
    background: var(--hover);
    color: var(--muted);
  }
  .seg {
    display: inline-flex;
    border: 1px solid var(--line);
    border-radius: 5px;
    overflow: hidden;
  }
  .tbtn {
    all: unset;
    font-size: 10.5px;
    padding: 1px 7px;
    color: var(--muted);
    border-right: 1px solid var(--line);
    cursor: default;
    white-space: nowrap;
  }
  .tbtn:last-child {
    border-right: 0;
  }
  .tbtn.solo {
    border: 1px solid var(--line);
    border-radius: 5px;
  }
  .tbtn:hover {
    background: var(--hover);
    color: var(--fg);
  }
  .tbtn.on {
    background: var(--sel);
    color: var(--fg);
    font-weight: 600;
  }
  .tbtn:focus-visible {
    outline: 2px solid var(--l0);
  }
  .vp {
    position: relative;
    min-height: 0;
    overflow: auto;
    padding: 36px 0 10px;
  }
  .chrome {
    position: absolute;
    top: 8px;
    right: 8px;
    z-index: 1;
    padding: 3px;
    border-radius: 7px;
    background: var(--panel);
    box-shadow: 0 1px 4px rgb(0 0 0 / 0.15);
  }
  .state {
    margin: 12px 10px;
    font-size: 11.5px;
  }
  .err {
    color: var(--bad-ink);
    font-size: 11.5px;
  }
  .sample-input {
    display: flex;
    align-items: center;
    gap: 6px;
    margin: 0 10px 8px;
    font-size: 10.5px;
    color: var(--muted);
  }
  .sample-input input {
    flex: 1;
    max-width: 420px;
    font: 12px inherit;
    padding: 2px 6px;
    border: 1px solid var(--line);
    border-radius: 4px;
    background: var(--bg);
    color: var(--fg);
  }
  .split {
    display: grid;
    grid-template-columns: 1fr 1fr;
  }
  .split.single {
    grid-template-columns: 1fr;
  }
  .col {
    position: relative;
    min-width: 0;
    padding: 24px 12px 8px;
  }
  .col + .col {
    border-left: 1px solid var(--line);
  }
  .lbl {
    position: absolute;
    top: 2px;
    left: 10px;
    font: 10.5px var(--mono);
    padding: 1px 7px;
    border-radius: 4px;
    background: var(--hover);
    color: var(--muted);
  }
  .big {
    font-size: 34px;
    line-height: 1.2;
    overflow-wrap: anywhere;
  }
  .small {
    margin: 6px 0 12px;
    font-size: 14px;
    overflow-wrap: anywhere;
  }
  .glyphs {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(32px, 1fr));
    gap: 3px;
  }
  .glyphs span {
    aspect-ratio: 1;
    display: grid;
    place-items: center;
    border: 1px solid var(--line);
    border-radius: 3px;
    font-size: 18px;
    background: var(--panel);
  }
  .glyphs .added {
    background: var(--add);
    border-color: var(--add-hi);
  }
  .glyphs .removed {
    background: var(--del);
    border-color: var(--del-hi);
  }
  .glyphs .changed {
    background: var(--warn);
    border-color: var(--warn-ink);
  }
  .more {
    margin-top: 8px;
  }
  .metrics {
    margin: 0 10px;
    border-collapse: collapse;
    font-size: 11.5px;
  }
  .metrics th,
  .metrics td {
    text-align: left;
    padding: 3px 14px 3px 0;
    border-bottom: 1px solid var(--line);
  }
  .metrics tbody th {
    color: var(--muted);
    font-weight: 400;
  }
  .metrics td {
    font-family: var(--mono);
  }
  .metrics tr.changed td {
    background: var(--warn);
    color: var(--warn-ink);
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
  .d-add {
    color: var(--add-ink);
  }
  .d-del {
    color: var(--bad-ink);
  }
  .d-mod {
    color: var(--warn-ink);
  }
  .axis {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    margin-left: 6px;
    font-family: var(--mono);
  }
  .axis input {
    width: 90px;
  }
</style>
