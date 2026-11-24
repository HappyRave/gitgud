<script lang="ts">
  import { untrack } from "svelte";
  import { type DataTables, dataTables } from "$lib/api";
  import type { ModelSide } from "$lib/model/open";
  import { toVersion } from "$lib/model/source";
  import { setViewerChoice, viewerChoice } from "$lib/prefs.svelte";
  import { ROW_FILTERS, type RowFilter, tableCaption } from "$lib/viewers/table";
  import LoadingBar from "./LoadingBar.svelte";

  let {
    repoId,
    path,
    before = null,
    after,
    bust = 0,
  }: {
    repoId: number;
    path: string;
    before?: ModelSide | null;
    after: ModelSide;
    bust?: number;
  } = $props();

  /** Rows drawn before "Show more". */
  const PAGE = 2000;

  let data = $state<DataTables | null>(null);
  let error = $state<string | null>(null);
  let tab = $state(0);
  let filter = $state<RowFilter>(untrack(() => viewerChoice(path, "rows", ROW_FILTERS)));
  let shown = $state(PAGE);
  let expanded = $state(false);
  let seq = 0;

  $effect(() => {
    const token = ++seq;
    void [repoId, path, bust, JSON.stringify(before?.at), JSON.stringify(after.at)];
    data = null;
    error = null;
    shown = PAGE;
    dataTables(repoId, path, before ? toVersion(before.at) : null, toVersion(after.at)).then(
      (d) => {
        if (token === seq) {
          data = d;
          tab = Math.min(untrack(() => tab), Math.max(0, d.tables.length - 1));
        }
      },
      (e) => {
        if (token === seq) {
          error = e instanceof Error ? e.message : String(e);
        }
      },
    );
  });

  const table = $derived(data?.tables[tab] ?? null);
  const compare = $derived(!!data?.compared);
  const visible = $derived(table ? (compare && filter === "changed" ? table.rows.filter((r) => r.state !== "same") : table.rows) : []);
  const hidden = $derived(table && compare && filter === "changed" ? table.rows.length - visible.length : 0);
  const removedCols = $derived(new Set(table?.removedColumns ?? []));
  const addedCols = $derived(new Set(table?.addedColumns ?? []));

  function pick(f: RowFilter) {
    filter = f;
    setViewerChoice(path, "rows", f);
  }
</script>

<div class="data" class:expanded>
  <div class="bar">
    {#if data && table}<span class="pill">{data.kind} · {table.rowCount.toLocaleString("en-US")} rows</span>{/if}
    <button type="button" class="tbtn solo" onclick={() => (expanded = !expanded)} aria-pressed={expanded}
      >{expanded ? "Close" : "Expand"}</button
    >
  </div>

  {#if error}
    <p class="state err" role="alert">{error}</p>
  {:else if !data}
    <p class="state"><LoadingBar label="Reading the table…" /></p>
  {:else if !table}
    <p class="state">No table in this file.</p>
  {:else}
    <div class="view">
      <div class="top">
        {#if data.tables.length > 1}
          <div class="seg" role="tablist" aria-label="Tables">
            {#each data.tables as t, i (t.name)}
              <button
                type="button"
                class="tbtn"
                class:on={tab === i}
                class:dirty={t.changed + t.added + t.removed > 0}
                role="tab"
                aria-selected={tab === i}
                onclick={() => ((tab = i), (shown = PAGE))}>{t.name}</button
              >
            {/each}
          </div>
        {/if}
        {#if compare}
          <div class="seg right" role="radiogroup" aria-label="Rows">
            {#each ROW_FILTERS as f (f)}
              <button type="button" class="tbtn" class:on={filter === f} role="radio" aria-checked={filter === f} onclick={() => pick(f)}
                >{f === "changed" ? "Changed rows" : "All rows"}</button
              >
            {/each}
          </div>
        {/if}
      </div>
      <div class="grid">
        <table>
          <thead>
            <tr>
              {#each table.columns as c (c)}
                <th class:add={addedCols.has(c)} class:del={removedCols.has(c)} class:key={c === table.key}>{c}</th>
              {/each}
            </tr>
          </thead>
          <tbody>
            {#each visible.slice(0, shown) as r, i (i)}
              <tr class={r.state}>
                {#each r.cells as [old, now], c (c)}
                  {#if r.state === "changed" && old !== null && now !== null && old !== now}
                    <td class="mod">{old} <span class="arrow">→</span> {now}</td>
                  {:else}
                    <td>{now ?? old ?? ""}</td>
                  {/if}
                {/each}
              </tr>
            {/each}
            {#if visible.length > shown}
              <tr class="note">
                <td colspan={table.columns.length}>
                  <button type="button" class="tbtn solo" onclick={() => (shown += PAGE)}
                    >Show {Math.min(PAGE, visible.length - shown).toLocaleString("en-US")} more of {(visible.length - shown).toLocaleString("en-US")}</button
                  >
                </td>
              </tr>
            {/if}
            {#if hidden > 0}
              <tr class="note"><td colspan={table.columns.length}>{hidden.toLocaleString("en-US")} unchanged rows hidden</td></tr>
            {/if}
          </tbody>
        </table>
      </div>
    </div>
    <div class="cap">
      {#each tableCaption(table, compare) as p, i (i)}
        {#if i > 0}<span>·</span>{/if}
        {#if i === 0}<b>{p.text}</b>{:else}<span class={p.tone ? `d-${p.tone}` : ""}>{p.text}</span>{/if}
      {/each}
      {#if table.truncated}<span>· only the first 20,000 rows are compared</span>{/if}
    </div>
  {/if}
</div>

<style>
  .data {
    display: grid;
    grid-template-rows: auto minmax(0, 1fr) auto;
    min-height: 260px;
    height: 100%;
    background: var(--panel);
  }
  .data.expanded {
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
  .state {
    margin: 12px 10px;
    font-size: 11.5px;
  }
  .err {
    color: var(--bad-ink);
  }
  .view {
    display: grid;
    grid-template-rows: auto minmax(0, 1fr);
    min-height: 0;
  }
  .top {
    display: flex;
    gap: 6px;
    padding: 6px 10px;
  }
  .right {
    margin-left: auto;
  }
  .grid {
    overflow: auto;
    min-height: 0;
  }
  table {
    border-collapse: collapse;
    font: 11px var(--mono);
    min-width: 100%;
  }
  th,
  td {
    text-align: left;
    padding: 3px 9px;
    border-bottom: 1px solid var(--line);
    border-right: 1px solid var(--line);
    white-space: pre;
    max-width: 320px;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  th {
    position: sticky;
    top: 0;
    z-index: 1;
    background: var(--panel);
    color: var(--muted);
    font-weight: 600;
  }
  th.key {
    color: var(--fg);
  }
  th.add {
    color: var(--add-ink);
  }
  th.del {
    color: var(--bad-ink);
    text-decoration: line-through;
  }
  td.mod {
    background: var(--warn);
    color: var(--warn-ink);
  }
  .arrow {
    opacity: 0.6;
  }
  tr.added td {
    background: var(--add);
  }
  tr.removed td {
    background: var(--del);
    text-decoration: line-through;
    opacity: 0.75;
  }
  tr.note td {
    color: var(--muted);
    font: 10.5px var(--sans, inherit);
    border-right: 0;
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
  .tbtn.dirty::after {
    content: " •";
    color: var(--warn-ink);
  }
  .tbtn:focus-visible {
    outline: 2px solid var(--l0);
  }
  .cap {
    display: flex;
    flex-wrap: wrap;
    gap: 5px;
    padding: 5px 10px;
    border-top: 1px solid var(--line);
    font-size: 11px;
    color: var(--muted);
  }
  .d-mod {
    color: var(--warn-ink);
  }
  .d-add {
    color: var(--add-ink);
  }
  .d-del {
    color: var(--bad-ink);
  }
</style>
