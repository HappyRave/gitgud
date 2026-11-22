<script lang="ts">
  import { onMount, tick } from "svelte";
  import {
    commitDetail,
    commitFileDiff,
    planPreview,
    predictedFile,
    type FileDiff,
    type HistoryPlan,
    type PlanAction,
    type PlanPrediction,
    type StepPrediction,
  } from "$lib/api";
  import { planKey } from "$lib/keys";
  import {
    ACTIONS,
    editsSquashMessage,
    firstConflict,
    folds,
    move,
    planRows,
    setAction,
    setMessage,
    steps,
    target,
    type PlanRow,
  } from "$lib/plan/plan";
  import DiffView from "./DiffView.svelte";

  let {
    repoId,
    plan,
    preset,
    branches = [],
    busy = false,
    onpreview,
    onstart,
    oncancel,
  }: {
    repoId: number;
    plan: HistoryPlan;
    /** One row's action set as the plan opens (R on an older commit). */
    preset?: { id: string; action: PlanAction };
    /** Local branches to offer as a new base. */
    branches?: string[];
    busy?: boolean;
    /** A new graph preview is ready: its row count. */
    onpreview: (rows: number) => void;
    onstart: (run: { onto: string; rows: PlanRow[]; updateRefs: boolean; prediction: PlanPrediction | null }) => void;
    oncancel: () => void;
  } = $props();

  // svelte-ignore state_referenced_locally
  let rows = $state<PlanRow[]>(planRows(plan, preset));
  // svelte-ignore state_referenced_locally
  let selected = $state(Math.max(0, preset ? rows.findIndex((r) => r.commit.id === preset.id) : 0));
  // svelte-ignore state_referenced_locally
  let onto = $state(plan.base);
  let updateRefs = $state(false);
  let prediction = $state<PlanPrediction | null>(null);
  let error = $state<string | null>(null);
  /** Why the last key did nothing, e.g. a squash with nothing to fold into. */
  let note = $state<string | null>(null);

  const KEYS = Object.fromEntries(ACTIONS.map((a) => [a.action, a]));
  const moved = $derived(onto !== plan.base);
  const conflict = $derived(prediction ? firstConflict(prediction) : null);
  const predictionOf = (id: string) => prediction?.steps.find((s) => s.id === id) ?? null;

  let request = 0;
  $effect(() => {
    const ask = { onto, list: steps(rows) };
    const mine = ++request;
    const timer = setTimeout(() => {
      planPreview(repoId, ask.onto, plan.base, ask.list).then(
        (p) => {
          if (mine === request) {
            prediction = p.prediction;
            error = null;
            onpreview(p.rows);
          }
        },
        (err) => {
          if (mine === request) {
            prediction = null;
            error = String(err);
          }
        },
      );
    }, 120);
    return () => clearTimeout(timer);
  });

  /** The selected commit's files, or the predicted conflict once "Show the conflict" is pressed. */
  let files = $state<FileDiff[]>([]);
  let showing = $state<{ step: number; path: string; text: string } | null>(null);
  const selectedId = $derived(rows[selected]?.commit.id);
  $effect(() => {
    const id = selectedId;
    files = [];
    if (!id) {
      return;
    }
    commitDetail(repoId, id).then(
      async (detail) => {
        const diffs = await Promise.all(detail.files.slice(0, 5).map((f) => commitFileDiff(repoId, id, f.path)));
        if (rows[selected]?.commit.id === id) {
          files = diffs;
        }
      },
      (err) => (error = String(err)),
    );
  });

  async function showConflict() {
    if (conflict?.outcome.kind !== "conflict" || conflict.step === null) {
      return;
    }
    const path = conflict.outcome.paths[0];
    try {
      showing = { step: conflict.step, path, text: await predictedFile(repoId, conflict.outcome.tree, path) };
    } catch (err) {
      error = String(err);
    }
  }

  /** Lines of a predicted file, tagged by the conflict-marker block they sit in. */
  function markedLines(text: string) {
    let side: "yours" | "theirs" | null = null;
    return text.split("\n").map((line) => {
      if (line.startsWith("<<<<<<<")) {
        side = "yours";
        return { line, side: "mark" };
      }
      if (line.startsWith("=======") && side) {
        side = "theirs";
        return { line, side: "mark" };
      }
      if (line.startsWith(">>>>>>>")) {
        side = null;
        return { line, side: "mark" };
      }
      return { line, side: side ?? "" };
    });
  }

  function apply(next: PlanRow[] | null, why: string) {
    if (next) {
      rows = next;
      note = null;
      showing = null;
    } else {
      note = why;
    }
  }

  function act(index: number, action: PlanAction) {
    selected = index;
    apply(setAction(rows, index, action), "The oldest commit has nothing older to fold into.");
    if (action === "reword" || action === "squash") {
      tick().then(() => list?.querySelector<HTMLTextAreaElement>(".pr.sel textarea")?.focus());
    }
  }

  function shift(by: -1 | 1) {
    const next = move(rows, selected, selected + by);
    if (next) {
      selected += by;
    }
    apply(next, "That would leave a fold at the bottom with nothing to fold into.");
  }

  const canStart = $derived(!busy && !error && rows.length > 0);

  function start() {
    if (canStart) {
      onstart({ onto, rows, updateRefs, prediction });
    }
  }

  let list = $state<HTMLElement | null>(null);
  onMount(() => list?.focus());

  function onKeydown(event: KeyboardEvent) {
    const key = planKey(event);
    // The Onto picker keeps its own arrows and letters.
    if (key && event.target instanceof HTMLSelectElement && key.kind !== "start" && key.kind !== "cancel") {
      return;
    }
    if (!key) {
      if (event.key === "Escape" && event.target instanceof HTMLTextAreaElement) {
        event.preventDefault();
        list?.focus();
      }
      return;
    }
    event.preventDefault();
    event.stopPropagation();
    switch (key.kind) {
      case "action":
        act(selected, key.action);
        break;
      case "move":
        shift(key.by);
        break;
      case "select":
        selected = Math.min(rows.length - 1, Math.max(0, selected + key.by));
        break;
      case "start":
        if (!conflict) {
          start();
        }
        break;
      case "cancel":
        oncancel();
        break;
    }
  }

  let dragFrom: number | null = null;

  function predText(p: StepPrediction | null): { text: string; cls: string } {
    if (!p) {
      return { text: "…", cls: "" };
    }
    switch (p.outcome.kind) {
      case "clean":
        return { text: "✓ clean", cls: "ok" };
      case "afterConflict":
        return { text: `✓ after step ${p.outcome.step}`, cls: "ok" };
      case "conflict":
        return { text: `⚠ ${p.outcome.paths.map((f) => f.split("/").pop()).join(", ")}`, cls: "bad" };
      case "dropped":
        return { text: "dropped", cls: "" };
    }
  }

  function sub(row: PlanRow, index: number): string {
    const p = predictionOf(row.commit.id);
    if (p?.outcome.kind === "conflict") {
      return `step ${p.step}: ${p.outcome.paths.join(", ")} would conflict${moved ? ` with ${onto}` : ""}`;
    }
    const into = folds(row.action) ? target(rows, index) : null;
    const intoId = into === null ? "" : rows[into].commit.shortId;
    switch (row.action) {
      case "reword":
        return row.message !== null && row.message !== row.commit.message ? `→ "${row.message.split("\n")[0]}"` : "";
      case "fixup":
        return `${row.commit.action === "fixup" ? 'auto from "fixup!": ' : ""}folds into ${intoId}, its own message is dropped`;
      case "squash":
        return `squashes into ${intoId}`;
      case "edit":
        return "stops here so you can change the commit";
      default:
        return "";
    }
  }

  const editing = (row: PlanRow, index: number) =>
    index === selected && (row.action === "reword" || (row.action === "squash" && editsSquashMessage(rows, index)));
</script>

<!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
<section class="plan" aria-label="Edit history" onkeydown={onKeydown}>
  <div class="ph">
    Edit history <span class="ct">· {plan.branch} · {rows.length} commit{rows.length === 1 ? "" : "s"} · newest first</span>
    <span class="r">Esc cancel</span>
  </div>
  <div class="chips">
    <label class="chip on" title="The commit the plan replays onto">
      Onto:
      <select bind:value={onto} aria-label="Onto">
        <option value={plan.base}>{plan.baseShort} · {plan.baseSubject}</option>
        {#each branches as b (b)}
          <option value={b}>{b}</option>
        {/each}
      </select>
    </label>
    <label class="chip" class:on={updateRefs} title={plan.dependents.length ? plan.dependents.join(", ") : "No other branch points into these commits"}>
      <input type="checkbox" bind:checked={updateRefs} disabled={plan.dependents.length === 0} />
      Update dependent branches ({plan.dependents.length})
    </label>
  </div>

  <div class="list" role="listbox" aria-label="Plan" tabindex="0" bind:this={list}>
    {#each rows as row, i (row.commit.id)}
      {@const p = predText(predictionOf(row.commit.id))}
      {@const a = KEYS[row.action]}
      {@const s = sub(row, i)}
      <!-- svelte-ignore a11y_click_events_have_key_events -->
      <div
        class="pr"
        class:sel={i === selected}
        class:dropped={row.action === "drop"}
        role="option"
        aria-selected={i === selected}
        tabindex="-1"
        draggable="true"
        ondragstart={() => (dragFrom = i)}
        ondragover={(e) => e.preventDefault()}
        ondrop={(e) => {
          e.preventDefault();
          if (dragFrom !== null) {
            selected = i;
            apply(move(rows, dragFrom, i), "That would leave a fold at the bottom with nothing to fold into.");
            dragFrom = null;
          }
        }}
        onclick={() => (selected = i)}
      >
        <span class="grip" aria-hidden="true">⋮⋮</span>
        <span class="act {row.action}" title="{a.label} ({a.key})"><b>{a.key}</b>{a.label}</span>
        <span class="ab">{row.commit.shortId}</span>
        <span class="t" title={row.commit.subject}>{row.commit.subject}</span>
        <span class="pred {p.cls}">{p.text}</span>
        {#if editing(row, i)}
          <textarea
            class="msg"
            aria-label="Message of {row.commit.shortId}"
            rows={Math.min(8, (row.message ?? "").split("\n").length + 1)}
            value={row.message ?? ""}
            oninput={(e) => (rows = setMessage(rows, i, e.currentTarget.value))}
          ></textarea>
          <span class="sub2">{row.action === "squash" ? "Squash keeps both messages: edit them into one." : "The new message."} Esc back to the list, Ctrl+↵ to start.</span>
        {:else if s}
          <span class="sub2" class:bad={predictionOf(row.commit.id)?.outcome.kind === "conflict"}>{s}</span>
        {/if}
      </div>
    {/each}
  </div>

  <div class="files">
    {#if showing}
      <div class="ph">Predicted conflict <span class="ct">· step {showing.step} · {showing.path}</span><button class="bare r" onclick={() => (showing = null)}>✕</button></div>
      <pre class="conflict" aria-label="Predicted conflict in {showing.path}">{#each markedLines(showing.text) as l, n (n)}<span class={l.side}>{l.line}
</span>{/each}</pre>
    {:else}
      {#each files as f (f.path)}
        <div class="ph">{rows[selected]?.commit.shortId} <span class="ct">· {f.path}</span></div>
        <DiffView diff={f} />
      {/each}
    {/if}
  </div>

  <div class="sum">
    {#if error}
      <div class="flag bad" role="alert">{error}</div>
    {/if}
    <div class="ln2">
      <span>Result: {rows.length} commit{rows.length === 1 ? "" : "s"} → {prediction?.after ?? "…"}</span>
      {#if conflict}
        {@const c = rows.find((r) => r.commit.id === conflict.id)}
        <span class="pred bad"
          >⚠ {prediction?.steps.filter((s) => s.outcome.kind === "conflict").length === 1 ? "1 conflict" : "conflicts"} predicted at step {conflict.step}
          ({c?.commit.subject} · {conflict.outcome.kind === "conflict" ? conflict.outcome.paths.join(", ") : ""})</span
        >
      {:else if prediction}
        <span class="pred ok">{prediction.changed === 0 ? "✓ final files identical to now" : `final files differ from now in ${prediction.changed} file${prediction.changed === 1 ? "" : "s"}`}</span>
        <span class="pred ok">✓ no conflicts predicted</span>
      {/if}
    </div>
    {#if plan.pushed}
      <div class="flag">{plan.branch} is on {plan.pushed}: after this you'll need to force-push (with lease).</div>
    {/if}
    {#if note}
      <div class="ab" role="status">{note}</div>
    {/if}
    <div class="ln2">
      {#if conflict}
        <button onclick={showConflict}>Show the conflict</button>
        {#if moved}<button onclick={() => (onto = plan.base)}>Keep the old base</button>{/if}
        <span class="sp"></span>
        <button onclick={oncancel}>Cancel <span class="k">Esc</span></button>
        <button disabled={!canStart} onclick={start}>Start anyway</button>
      {:else}
        <span class="ab keys">P pick · R reword · E edit · S squash · F fixup · D drop · Alt+↑↓ move</span>
        <span class="sp"></span>
        <button onclick={oncancel}>Cancel <span class="k">Esc</span></button>
        <button class="pri" disabled={!canStart} onclick={start}>Start rebase <span class="k">Ctrl+↵</span></button>
      {/if}
    </div>
  </div>
</section>

<style>
  .plan {
    flex: 1;
    display: flex;
    flex-direction: column;
    min-height: 0;
  }
  .chips {
    display: flex;
    gap: 6px;
    padding: 6px 10px;
    border-bottom: 1px solid var(--line);
    flex-wrap: wrap;
    flex: none;
  }
  .chip {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    font-size: 11px;
    padding: 1px 8px;
    border-radius: 999px;
    border: 1px solid var(--line);
    color: var(--muted);
    white-space: nowrap;
  }
  .chip.on {
    background: var(--sel);
    color: var(--fg);
    border-color: transparent;
  }
  .chip select {
    font: inherit;
    color: inherit;
    background: transparent;
    border: 0;
    max-width: 280px;
  }
  .chip input {
    margin: 0;
  }
  .list {
    flex: none;
    max-height: 55%;
    overflow-y: auto;
  }
  .list:focus {
    outline: none;
  }
  .list:focus-visible .pr.sel {
    box-shadow: inset 2px 0 0 var(--l0);
  }
  .pr {
    display: grid;
    grid-template-columns: 14px 74px 58px minmax(0, 1fr) auto;
    gap: 8px;
    align-items: center;
    min-height: 30px;
    padding: 3px 10px;
    border-bottom: 1px solid var(--line);
    cursor: default;
  }
  .pr.sel {
    background: var(--sel);
  }
  .pr.dropped .t {
    text-decoration: line-through;
    color: var(--muted);
  }
  .grip {
    color: var(--muted);
    letter-spacing: -2px;
    cursor: grab;
  }
  .t {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .act {
    display: inline-flex;
    gap: 5px;
    align-items: center;
    font: 600 11px var(--body);
    padding: 2px 6px;
    border-radius: 4px;
    border: 1px solid var(--line);
  }
  .act b {
    font: 600 10px var(--mono);
    border-radius: 3px;
    padding: 0 4px;
    background: var(--hover);
  }
  .act.reword,
  .act.edit {
    color: var(--l0);
    border-color: currentColor;
  }
  .act.fixup,
  .act.squash {
    color: var(--l4);
    border-color: currentColor;
  }
  .act.drop {
    color: var(--bad-ink);
    border-color: currentColor;
  }
  .pred {
    font-size: 11px;
    white-space: nowrap;
    color: var(--muted);
  }
  .pred.ok {
    color: var(--add-ink);
  }
  .pred.bad {
    color: var(--bad-ink);
    font-weight: 600;
    white-space: normal;
  }
  .sub2 {
    grid-column: 3 / -1;
    font-size: 11px;
    color: var(--muted);
  }
  .sub2.bad {
    color: var(--bad-ink);
  }
  .msg {
    grid-column: 3 / -1;
    border: 1.5px solid var(--l0);
    border-radius: 5px;
    padding: 6px 8px;
    font: 11.5px/1.5 var(--mono);
    background: var(--panel);
    resize: vertical;
  }
  .files {
    flex: 1;
    min-height: 0;
    overflow: auto;
    border-top: 1px solid var(--line);
    margin-top: 6px;
  }
  .files .r {
    all: unset;
    margin-left: auto;
    cursor: default;
  }
  .conflict {
    margin: 0;
    font: 12px/18px var(--mono);
    white-space: pre;
  }
  .conflict span {
    display: block;
    padding: 0 8px;
  }
  .conflict .yours {
    background: var(--yours-bg);
  }
  .conflict .theirs {
    background: var(--theirs-bg);
  }
  .conflict .mark {
    color: var(--muted);
  }
  .sum {
    border-top: 1px solid var(--line);
    padding: 8px 10px;
    display: grid;
    gap: 6px;
    flex: none;
  }
  .ln2 {
    display: flex;
    gap: 8px;
    align-items: center;
    flex-wrap: wrap;
  }
  .sp {
    flex: 1;
  }
  .keys {
    white-space: normal;
  }
</style>
