<script lang="ts">
  import { untrack } from "svelte";
  import { SvelteMap } from "svelte/reactivity";
  import {
    propertyMerge,
    propertyMergeCheck,
    propertyMergeDraft,
    type MergeCheck,
    type MergeRow,
    type PropertyMerge,
    type RowPick,
  } from "$lib/api";
  import { inTextField } from "$lib/keys";
  import {
    type Choice,
    type Filter,
    choiceOf,
    matches,
    openClashes,
    picksSince,
    resultSources,
    resultText,
    toPicks,
  } from "$lib/merge/property";
  import { TINT } from "$lib/model/compare";
  import ModelViewer from "./ModelViewer.svelte";

  let {
    repoId,
    path,
    revision,
    yoursLabel,
    theirsLabel,
    busy = false,
    onresolve,
    onrawyaml,
    onback,
    onfallbacktext,
  }: {
    repoId: number;
    path: string;
    revision: number;
    yoursLabel: string;
    theirsLabel: string;
    busy?: boolean;
    onresolve: (picks: RowPick[]) => void;
    /** Text Unity YAML: switch to the raw three-column text merge. */
    onrawyaml?: () => void;
    /** LFS files: back to picking a side. */
    onback?: () => void;
    /** The file can't be merged by property: fall back to another resolver. */
    onfallbacktext: (reason: string) => void;
  } = $props();

  /** One-sided rows shown before "Show all". */
  const SHOWN = 400;

  let merge = $state<PropertyMerge | null>(null);
  const picks = new SvelteMap<string, Choice>();
  let filter = $state<Filter>("all");
  let currentId = $state<string | null>(null);
  let editing = $state<{ id: string; value: string } | null>(null);
  let showAll = $state(false);
  let check = $state<MergeCheck | null>(null);
  let checkError = $state<string | null>(null);
  /** The 3D result pane (R9, A: built on demand from the picks of that moment). */
  let resultOpen = $state(false);
  let built = $state<{ picks: Map<string, Choice>; at: string; bust: number } | null>(null);
  let building = $state(false);
  let buildError = $state<string | null>(null);
  let buildSeq = 0;
  const has3d = $derived(/\.(unity|prefab)$/i.test(path));

  const rows = $derived(merge?.rows ?? []);
  const visible = $derived(rows.filter((r) => matches(r, filter)));
  const clashes = $derived(visible.filter((r) => r.source === "clash"));
  const oneSidedAll = $derived(visible.filter((r) => r.source !== "clash"));
  const oneSided = $derived(showAll ? oneSidedAll : oneSidedAll.slice(0, SHOWN));
  const navigable = $derived([...clashes, ...oneSided]);
  const left = $derived(openClashes(rows, picks));
  const totalClashes = $derived(rows.filter((r) => r.source === "clash").length);
  const current = $derived(navigable.find((r) => r.id === currentId) ?? null);
  const kept = $derived(
    rows.reduce(
      (acc, r) => {
        const c = choiceOf(r, picks);
        if (c === "yours") {
          acc.yours += 1;
        } else if (c === "theirs") {
          acc.theirs += 1;
        } else if (c) {
          acc.edited += 1;
        }
        return acc;
      },
      { yours: 0, theirs: 0, edited: 0 },
    ),
  );

  $effect(() => {
    const key = `${repoId}:${path}:${revision}`;
    let stale = false;
    untrack(() => {
      editing = null;
    });
    propertyMerge(repoId, path).then(
      (m) => {
        if (stale || key !== `${repoId}:${path}:${revision}`) {
          return;
        }
        merge = m;
        picks.clear();
        filter = "all";
        showAll = false;
        currentId = m.rows[0]?.id ?? null;
        built = null;
        if (resultOpen) {
          buildResult();
        }
      },
      (err) => {
        if (!stale) {
          onfallbacktext(String(err));
        }
      },
    );
    return () => {
      stale = true;
    };
  });

  // The integrity check runs on the result once every clash has a pick.
  $effect(() => {
    if (!merge || left > 0) {
      check = null;
      return;
    }
    const sent = toPicks(rows, picks);
    let stale = false;
    const timer = setTimeout(() => {
      propertyMergeCheck(repoId, path, sent).then(
        (c) => {
          if (!stale) {
            check = c;
            checkError = null;
          }
        },
        (err) => {
          if (!stale) {
            checkError = String(err);
          }
        },
      );
    }, 250);
    return () => {
      stale = true;
      clearTimeout(timer);
    };
  });

  const since = $derived(built ? picksSince(rows, built.picks, picks) : 0);
  const sources = $derived(built ? resultSources(rows, built.picks) : null);

  function buildResult() {
    if (!merge) {
      return;
    }
    const snapshot = new Map(picks);
    const seq = ++buildSeq;
    building = true;
    buildError = null;
    propertyMergeDraft(repoId, path, toPicks(rows, snapshot)).then(
      () => {
        if (seq !== buildSeq) {
          return;
        }
        const at = new Date().toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" });
        built = { picks: snapshot, at, bust: (built?.bust ?? 0) + 1 };
        building = false;
      },
      (err) => {
        if (seq === buildSeq) {
          buildError = String(err);
          building = false;
        }
      },
    );
  }

  function toggleResult() {
    resultOpen = !resultOpen;
    if (resultOpen && (!built || since > 0)) {
      buildResult();
    }
  }

  function choose(row: MergeRow, choice: Choice) {
    currentId = row.id;
    picks.set(row.id, choice);
    if (row.source === "clash") {
      const at = navigable.indexOf(row);
      const next = navigable.find((r, i) => i > at && r.source === "clash" && !picks.has(r.id));
      if (next) {
        currentId = next.id;
      }
    }
  }

  /** Every clash to one side; one-sided changes keep their own. */
  function pickAll(side: "yours" | "theirs") {
    for (const row of rows) {
      if (row.source === "clash") {
        picks.set(row.id, side);
      }
    }
  }

  function startEdit(row: MergeRow) {
    if (!row.editable) {
      return;
    }
    currentId = row.id;
    const c = choiceOf(row, picks);
    const value =
      typeof c === "object" && c !== null ? c.custom : ((c === "theirs" ? row.theirsRaw : row.yoursRaw) ?? "").trim();
    editing = { id: row.id, value };
  }

  function commitEdit(row: MergeRow) {
    if (editing?.id !== row.id) {
      return;
    }
    picks.set(row.id, { custom: editing.value });
    editing = null;
  }

  function save() {
    if (!merge || left > 0 || busy) {
      return;
    }
    onresolve(toPicks(rows, picks));
  }

  function move(step: number) {
    if (navigable.length === 0) {
      return;
    }
    const at = current ? navigable.indexOf(current) : -1;
    const next = navigable[Math.max(0, Math.min(navigable.length - 1, at + step))];
    currentId = next.id;
    document.getElementById(`pm-${next.id}`)?.scrollIntoView({ block: "nearest" });
  }

  function onKeydown(event: KeyboardEvent) {
    if (editing || event.altKey || event.metaKey || inTextField(event.target)) {
      return;
    }
    if (event.ctrlKey) {
      if (event.key === "Enter") {
        event.preventDefault();
        save();
      }
      return;
    }
    if ((event.key === "r" || event.key === "R") && resultOpen && !building) {
      event.preventDefault();
      buildResult();
      return;
    }
    if (event.key === "ArrowDown" || event.key === "ArrowUp") {
      event.preventDefault();
      move(event.key === "ArrowDown" ? 1 : -1);
      return;
    }
    if (!current) {
      return;
    }
    if (event.key === "1" || event.key === "2") {
      event.preventDefault();
      choose(current, event.key === "1" ? "yours" : "theirs");
    } else if (event.key === "e" || event.key === "E") {
      if (current.editable) {
        event.preventDefault();
        startEdit(current);
      }
    }
  }

  const FILTERS: { id: Filter; label: string; count: (m: PropertyMerge) => number }[] = [
    { id: "all", label: "All", count: (m) => m.rows.length },
    { id: "clashes", label: "Clashes", count: (m) => m.clashes },
    { id: "yours", label: "Yours", count: (m) => m.yours },
    { id: "theirs", label: "Theirs", count: (m) => m.theirs },
    { id: "objects", label: "Objects added / deleted", count: (m) => m.objects },
  ];

  const sideName = (side: "yours" | "theirs") => (side === "yours" ? yoursLabel : theirsLabel);
</script>

<svelte:window onkeydown={onKeydown} />

{#snippet valueCell(row: MergeRow, side: "yours" | "theirs")}
  {@const text = side === "yours" ? row.yours : row.theirs}
  {@const chosen = choiceOf(row, picks) === side}
  {@const unchanged = row.source !== "clash" && row.source !== side}
  <button
    type="button"
    class="v {side}"
    class:chosen
    class:same={unchanged && !chosen}
    title={text ?? "absent"}
    aria-label="{sideName(side)}: {text ?? 'absent'}"
    onclick={(e) => {
      e.stopPropagation();
      choose(row, side);
    }}>{text ?? "—"}</button
  >
{/snippet}

{#snippet tableRow(row: MergeRow)}
  {@const choice = choiceOf(row, picks)}
  {@const result = resultText(row, choice)}
  <tr
    id="pm-{row.id}"
    class:clash={row.source === "clash"}
    class:active={row.id === currentId}
    onclick={() => (currentId = row.id)}
  >
    <td class="prop">
      <div class="obj">{row.objectPath}{#if row.component}<span class="comp"> › {row.component}</span>{/if}</div>
      <div class="pname mono">{row.property}</div>
    </td>
    <td><span class="v base" title={row.base ?? "absent"}>{row.base ?? "—"}</span></td>
    <td>{@render valueCell(row, "yours")}</td>
    <td>{@render valueCell(row, "theirs")}</td>
    <td class="res">
      {#if editing?.id === row.id}
        <span class="editrow">
          <input
            type="text"
            aria-label="Edit {row.property}"
            bind:value={editing.value}
            onkeydown={(e) => {
              if (e.key === "Enter") {
                e.preventDefault();
                commitEdit(row);
              } else if (e.key === "Escape") {
                e.preventDefault();
                editing = null;
              }
            }}
          />
          <button type="button" class="sm" onclick={() => commitEdit(row)}>Set</button>
        </span>
      {:else if result === null}
        <span class="v need">pick 1 · 2{row.editable ? " · E" : ""}</span>
      {:else}
        <span class="v out" title={result}>{result}</span>
        {#if typeof choice === "object"}
          <span class="chip edited">edited</span>
        {:else if choice}
          <span class="chip {choice}">{choice}</span>
        {/if}
        {#if row.editable}
          <button
            type="button"
            class="edit"
            aria-label="Edit {row.property}"
            title="Edit (E)"
            onclick={(e) => {
              e.stopPropagation();
              startEdit(row);
            }}>E</button
          >
        {/if}
      {/if}
    </td>
  </tr>
  {#if row.id === currentId && (row.kind === "order" || row.yoursDetail.length > 0 || row.theirsDetail.length > 0)}
    <tr class="detail">
      <td colspan="5">
        {#if row.kind === "order"}
          <p class="dh">Both of you added items here. Both are kept; which comes first?</p>
        {/if}
        <div class="sides">
          {#each ["yours", "theirs"] as const as side (side)}
            {@const lines = side === "yours" ? row.yoursDetail : row.theirsDetail}
            {#if lines.length > 0}
              <button type="button" class="card {side}" class:chosen={choiceOf(row, picks) === side} onclick={() => choose(row, side)}>
                <b>
                  {#if row.kind === "order"}
                    {side === "yours" ? "Yours" : "Theirs"} first
                  {:else}
                    {sideName(side)}'s changes to it
                  {/if}
                  <span class="k">{side === "yours" ? 1 : 2}</span>
                </b>
                {#each lines as line, i (i)}
                  <span class="mono">{line}</span>
                {/each}
              </button>
            {/if}
          {/each}
        </div>
      </td>
    </tr>
  {/if}
{/snippet}

<section class="umt" aria-label="Merge by property for {path}">
  <div class="ph">
    <span class="name" title={path}>{path}</span>
    <span class="ct">· merge by property{merge ? ` · ${totalClashes - left}/${totalClashes} clashes picked` : ""}</span>
    <span class="r actions">
      {#if totalClashes > 0}
        <button type="button" class="sm" onclick={() => pickAll("yours")}>Pick all {yoursLabel}</button>
        <button type="button" class="sm" onclick={() => pickAll("theirs")}>Pick all {theirsLabel}</button>
      {/if}
      {#if onback}
        <button type="button" class="sm" onclick={onback}>Back to pick a side</button>
      {/if}
      {#if onrawyaml}
        <button type="button" class="sm" onclick={onrawyaml}>Raw YAML</button>
      {/if}
      {#if has3d}
        <button type="button" class="sm" disabled={!merge} aria-pressed={resultOpen} onclick={toggleResult}>Result in 3D</button>
      {/if}
      <button type="button" class="pri sm" disabled={!merge || left > 0 || busy} onclick={save}>
        Save &amp; mark resolved
      </button>
    </span>
  </div>

  {#if !merge}
    <p class="note">Reading the three versions…</p>
  {:else}
    <div class="body" class:split={resultOpen}>
    <div class="left">
    <div class="pills" role="group" aria-label="Show">
      {#each FILTERS as f (f.id)}
        <button type="button" class="pill" class:on={filter === f.id} aria-pressed={filter === f.id} onclick={() => (filter = f.id)}>
          {f.label} <span class="n">{f.count(merge)}</span>
        </button>
      {/each}
    </div>
    <div class="scroll">
      {#if rows.length === 0}
        <p class="note">Both sides made the same changes: save to take them.</p>
      {:else}
        <table class="tbl" aria-label="Changes">
          <thead>
            <tr>
              <th>Object › Component › Property</th>
              <th>Base</th>
              <th class="col-yours">{yoursLabel} <span class="k">1</span></th>
              <th class="col-theirs">{theirsLabel} <span class="k">2</span></th>
              <th>Result <span class="k">E</span></th>
            </tr>
          </thead>
          <tbody>
            {#if clashes.length > 0}
              <tr class="grp-row"><td colspan="5">Clashes · {left} to pick</td></tr>
              {#each clashes as row (row.id)}
                {@render tableRow(row)}
              {/each}
            {/if}
            {#if oneSidedAll.length > 0}
              <tr class="grp-row"><td colspan="5">Changed on one side · applied · {oneSidedAll.length}</td></tr>
              {#each oneSided as row (row.id)}
                {@render tableRow(row)}
              {/each}
              {#if oneSided.length < oneSidedAll.length}
                <tr class="more">
                  <td colspan="5">
                    {oneSidedAll.length - oneSided.length} more
                    <button type="button" class="sm" onclick={() => (showAll = true)}>Show all</button>
                  </td>
                </tr>
              {/if}
            {/if}
          </tbody>
        </table>
      {/if}

      {#if left === 0}
        <section class="summary" aria-label="Merge summary">
          <b>The result keeps {kept.yours} change{kept.yours === 1 ? "" : "s"} from {yoursLabel}, {kept.theirs} from {theirsLabel}{kept.edited ? `, ${kept.edited} edited` : ""}.</b>
          {#if checkError}
            <span class="bad">{checkError}</span>
          {:else if !check}
            <span class="muted">Checking the result…</span>
          {:else if check.issues.length === 0}
            <span class="ok">✓ No broken references or duplicate fileIDs</span>
          {:else}
            <span class="bad">⚠ {check.issues.length} issue{check.issues.length === 1 ? "" : "s"} in the result:</span>
            <ul>
              {#each check.issues as issue, i (i)}
                <li>{issue.detail}</li>
              {/each}
            </ul>
          {/if}
          <span class="muted">Saving writes the file and stages it{onback ? "; git stores it in LFS again" : ""}. Ctrl+Z undoes it.</span>
        </section>
      {/if}
    </div>
    </div>
    {#if resultOpen}
      <section class="result" aria-label="Merged result in 3D">
        <div class="ph">
          <span class="name">Result{built ? ` · built ${built.at}` : ""}</span>
          {#if building}<span class="ct">· building…</span>{/if}
        </div>
        {#if since > 0 && !building}
          <div class="stale" role="status">
            Out of date · {since} pick{since === 1 ? "" : "s"} since
            <span class="grow"></span>
            <button type="button" class="sm" onclick={buildResult}>Rebuild <span class="k">R</span></button>
          </div>
        {/if}
        {#if buildError}
          <p class="note bad">{buildError}</p>
        {:else if built}
          <div class="viewer">
            <ModelViewer {repoId} {path} after={{ label: "Merged result", at: { at: "draft" } }} bust={built.bust} {sources} />
          </div>
          <div class="legend" aria-label="Tint">
            <span><i style:background={TINT.yours}></i>from {yoursLabel}</span>
            <span><i style:background={TINT.theirs}></i>from {theirsLabel}</span>
            <span><i style:background={TINT.clash}></i>clash</span>
            <span class="muted">white: unchanged · Changes shading</span>
          </div>
        {:else}
          <p class="note">Building the result…</p>
        {/if}
      </section>
    {/if}
    </div>
  {/if}
</section>

<style>
  .umt {
    display: flex;
    flex-direction: column;
    min-height: 0;
    height: 100%;
  }
  .name {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    text-transform: none;
    letter-spacing: 0;
  }
  .actions {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    flex: none;
  }
  .sm {
    height: 22px;
    padding: 0 8px;
    font-size: 11px;
  }
  .pills {
    display: flex;
    gap: 5px;
    padding: 6px 10px;
    border-bottom: 1px solid var(--line);
    flex-wrap: wrap;
  }
  .pill {
    height: 22px;
    padding: 0 9px;
    border-radius: 11px;
    font-size: 11.5px;
    color: var(--muted);
    white-space: nowrap;
  }
  .pill.on {
    color: var(--fg);
  }
  .pill .n {
    font: 10.5px var(--mono);
    margin-left: 3px;
  }
  .scroll {
    flex: 1;
    overflow: auto;
    padding: 0 0 10px;
  }
  .body {
    flex: 1;
    min-height: 0;
    display: grid;
    grid-template-columns: minmax(0, 1fr);
  }
  .body.split {
    grid-template-columns: minmax(0, 1.4fr) minmax(320px, 1fr);
  }
  .left,
  .result {
    display: flex;
    flex-direction: column;
    min-height: 0;
    min-width: 0;
  }
  .result {
    border-left: 1px solid var(--line);
  }
  .result .viewer {
    flex: 1;
    min-height: 0;
    display: flex;
  }
  .stale {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 4px 10px;
    background: var(--warn);
    color: var(--warn-ink);
    font-size: 11.5px;
  }
  .grow {
    flex: 1;
  }
  .legend {
    display: flex;
    flex-wrap: wrap;
    gap: 4px 12px;
    padding: 5px 10px;
    border-top: 1px solid var(--line);
    font-size: 11px;
  }
  .legend i {
    display: inline-block;
    width: 9px;
    height: 9px;
    border-radius: 2px;
    margin-right: 5px;
    vertical-align: -1px;
    box-shadow: inset 0 0 0 1px color-mix(in srgb, var(--fg) 25%, transparent);
  }
  .tbl {
    width: 100%;
    border-collapse: collapse;
    font-size: 11.5px;
  }
  .tbl th {
    position: sticky;
    top: 0;
    z-index: 1;
    background: var(--panel);
    text-align: left;
    padding: 5px 8px;
    font-weight: 600;
    color: var(--muted);
    border-bottom: 1px solid var(--line);
    white-space: nowrap;
  }
  .tbl td {
    padding: 4px 8px;
    border-bottom: 1px solid var(--line);
    vertical-align: middle;
  }
  .tbl tr.active td {
    background: var(--hover);
  }
  .tbl tr.clash td:first-child {
    box-shadow: inset 3px 0 var(--bad-ink);
  }
  .grp-row td {
    background: var(--bg);
    font: 600 10px var(--mono);
    letter-spacing: 0.05em;
    text-transform: uppercase;
    color: var(--muted);
  }
  .tbl th.col-yours {
    color: var(--yours);
  }
  .tbl th.col-theirs {
    color: var(--theirs);
  }
  .obj {
    font-weight: 600;
    overflow-wrap: anywhere;
  }
  .comp {
    font-weight: 400;
    color: var(--muted);
  }
  .pname {
    color: var(--muted);
    font-size: 11px;
  }
  .v {
    display: inline-block;
    max-width: 260px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    vertical-align: middle;
    font: 11.5px var(--mono);
    padding: 1px 6px;
    border-radius: 3px;
    border: 1px solid transparent;
    background: none;
    text-align: left;
  }
  .v.base,
  .v.same {
    color: var(--muted);
  }
  .v.same {
    opacity: 0.6;
  }
  button.v {
    cursor: pointer;
    height: auto;
  }
  button.v:hover {
    border-color: var(--line);
  }
  .v.yours {
    color: var(--yours);
  }
  .v.theirs {
    color: var(--theirs);
  }
  .v.yours.chosen {
    background: var(--yours-bg);
    border-color: var(--yours);
  }
  .v.theirs.chosen {
    background: var(--theirs-bg);
    border-color: var(--theirs);
  }
  .v.need {
    color: var(--bad-ink);
    border: 1px dashed var(--bad-ink);
  }
  .res {
    white-space: nowrap;
  }
  .chip {
    font: 600 9.5px var(--mono);
    padding: 0 5px;
    border-radius: 8px;
    margin-left: 4px;
    vertical-align: middle;
  }
  .chip.yours {
    background: var(--yours-bg);
    color: var(--yours);
  }
  .chip.theirs {
    background: var(--theirs-bg);
    color: var(--theirs);
  }
  .chip.edited {
    background: var(--hover);
    color: var(--fg);
  }
  .edit {
    height: 18px;
    padding: 0 5px;
    margin-left: 4px;
    font: 10px var(--mono);
    color: var(--muted);
  }
  .editrow {
    display: inline-flex;
    align-items: center;
    gap: 6px;
  }
  .editrow input {
    min-width: 90px;
    height: 22px;
    font: 11px var(--mono);
  }
  .detail td {
    background: var(--hover);
  }
  .dh {
    margin: 2px 0 6px;
    font-weight: 600;
  }
  .sides {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 8px;
  }
  .card {
    display: grid;
    gap: 2px;
    text-align: left;
    padding: 6px 9px;
    border: 1px solid var(--line);
    border-radius: 5px;
    background: var(--panel);
    height: auto;
    cursor: pointer;
  }
  .card.yours {
    border-top: 2px solid var(--yours);
  }
  .card.theirs {
    border-top: 2px solid var(--theirs);
  }
  .card.chosen.yours {
    background: var(--yours-bg);
  }
  .card.chosen.theirs {
    background: var(--theirs-bg);
  }
  .more td {
    color: var(--muted);
  }
  .summary {
    display: grid;
    gap: 4px;
    margin: 12px 10px 0;
    padding: 10px 12px;
    border: 1px solid var(--line);
    border-radius: 6px;
    background: var(--panel);
    font-size: 12px;
  }
  .summary ul {
    margin: 0;
    padding-left: 18px;
  }
  .ok {
    color: var(--add-ink);
  }
  .bad {
    color: var(--bad-ink);
  }
  .muted {
    color: var(--muted);
  }
  .mono {
    font-family: var(--mono);
  }
  .note {
    margin: 10px;
    color: var(--muted);
  }
</style>
