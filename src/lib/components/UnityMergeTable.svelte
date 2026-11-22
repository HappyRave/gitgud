<script lang="ts">
  import { untrack } from "svelte";
  import { SvelteMap } from "svelte/reactivity";
  import {
    unityConflictTable,
    type UnityConflictRow,
    type UnityConflictTable,
    type UnityPropertyPick,
  } from "$lib/api";

  let {
    repoId,
    path,
    revision,
    yoursLabel,
    theirsLabel,
    busy = false,
    onresolve,
    onrawyaml,
    onfallbacktext,
  }: {
    repoId: number;
    path: string;
    revision: number;
    yoursLabel: string;
    theirsLabel: string;
    busy?: boolean;
    onresolve: (picks: UnityPropertyPick[]) => void;
    onrawyaml: () => void;
    onfallbacktext: () => void;
  } = $props();

  let table = $state<UnityConflictTable | null>(null);
  let error = $state<string | null>(null);
  const picks = new SvelteMap<string, { choice: "yours" | "theirs" | "custom"; value: string | null }>();
  let current = $state(0);
  let editing = $state<{ id: string; value: string } | null>(null);

  const conflicts = $derived(table?.conflicts ?? []);
  const pickedCount = $derived(conflicts.filter((r) => picks.has(r.id)).length);
  const allPicked = $derived(table !== null && pickedCount === conflicts.length);

  $effect(() => {
    const key = `${repoId}:${path}:${revision}`;
    let stale = false;
    untrack(() => {
      error = null;
      editing = null;
    });
    unityConflictTable(repoId, path).then(
      (t) => {
        if (stale || key !== `${repoId}:${path}:${revision}`) {
          return;
        }
        table = t;
        picks.clear();
        current = 0;
      },
      (err) => {
        if (stale) {
          return;
        }
        const msg = String(err);
        if (
          msg.includes("not valid multi-document Unity YAML") ||
          msg.includes("pick a side") ||
          msg.includes("binary") ||
          msg.includes("not a Unity YAML")
        ) {
          onfallbacktext();
        } else {
          error = msg;
        }
      },
    );
    return () => {
      stale = true;
    };
  });

  function chooseSide(row: UnityConflictRow, choice: "yours" | "theirs", idx: number) {
    current = idx;
    picks.set(row.id, { choice, value: choice === "yours" ? row.yours : row.theirs });
    const nextOpen = conflicts.findIndex((r, i) => i > idx && !picks.has(r.id));
    if (nextOpen >= 0) {
      current = nextOpen;
    }
  }

  function pickAll(choice: "yours" | "theirs") {
    for (const row of conflicts) {
      picks.set(row.id, { choice, value: choice === "yours" ? row.yours : row.theirs });
    }
  }

  function startEdit(row: UnityConflictRow, idx: number) {
    current = idx;
    const existing = picks.get(row.id);
    editing = { id: row.id, value: existing?.value ?? row.yours ?? row.theirs ?? row.base ?? "" };
  }

  function commitEdit(row: UnityConflictRow) {
    if (!editing || editing.id !== row.id) {
      return;
    }
    const val = editing.value;
    picks.set(row.id, { choice: "custom", value: val });
    editing = null;
  }

  function save() {
    if (!table || !allPicked || busy) {
      return;
    }
    const out: UnityPropertyPick[] = table.conflicts.map((row) => ({
      fileId: row.fileId,
      property: row.property,
      value: picks.get(row.id)?.value ?? row.yours,
    }));
    onresolve(out);
  }

  function onKeydown(event: KeyboardEvent) {
    if (conflicts.length === 0 || editing) {
      return;
    }
    const el = event.target as HTMLElement | null;
    if (el && (el.tagName === "INPUT" || el.tagName === "TEXTAREA" || el.isContentEditable)) {
      return;
    }
    const row = conflicts[current];
    if (!row) {
      return;
    }
    if (event.key === "1") {
      event.preventDefault();
      chooseSide(row, "yours", current);
    } else if (event.key === "2") {
      event.preventDefault();
      chooseSide(row, "theirs", current);
    } else if (event.key === "e" || event.key === "E") {
      event.preventDefault();
      startEdit(row, current);
    } else if (event.key === "ArrowDown") {
      event.preventDefault();
      current = Math.min(conflicts.length - 1, current + 1);
    } else if (event.key === "ArrowUp") {
      event.preventDefault();
      current = Math.max(0, current - 1);
    }
  }
</script>

<svelte:window onkeydown={onKeydown} />

<section class="umt" aria-label="Unity property merge for {path}">
  <div class="ph">
    <span class="name mono" title={path}>{path}</span>
    <span class="ct">· {pickedCount}/{conflicts.length} properties picked</span>
    <span class="r actions">
      {#if conflicts.length > 0}
        <button type="button" class="sm" disabled={busy} onclick={() => pickAll("yours")}>Pick all {yoursLabel}</button>
        <button type="button" class="sm" disabled={busy} onclick={() => pickAll("theirs")}>Pick all {theirsLabel}</button>
      {/if}
      <button type="button" class="sm" onclick={onrawyaml}>Raw YAML</button>
      <button type="button" class="pri sm" disabled={!allPicked || busy} onclick={save}>Save &amp; stage</button>
    </span>
  </div>

  {#if error}
    <p class="err" role="alert">{error}</p>
  {:else if !table}
    <p class="note">Parsing Unity YAML properties…</p>
  {:else}
    <div class="scroll">
      {#if conflicts.length === 0}
        <p class="note">No conflicting properties remain; save to apply auto-merged changes and stage.</p>
      {:else}
        <table class="tbl" aria-label="Property conflicts">
          <thead>
            <tr>
              <th>Object › Component › Property</th>
              <th>Base</th>
              <th>{yoursLabel} <span class="k">1</span></th>
              <th>{theirsLabel} <span class="k">2</span></th>
              <th>Result <span class="k">E</span></th>
            </tr>
          </thead>
          <tbody>
            {#each conflicts as row, idx (row.id)}
              {@const picked = picks.get(row.id)}
              <tr class:active={idx === current} onclick={() => (current = idx)}>
                <td class="prop">
                  <div class="obj">{row.objectPath} <span class="comp">› {row.component}</span></div>
                  <div class="pname mono">{row.property}</div>
                </td>
                <td class="base mono" title={row.base ?? "∅"}>{row.baseDisplay ?? row.base ?? "∅"}</td>
                <td class="side">
                  <button
                    type="button"
                    class="pick"
                    class:chosen={picked?.choice === "yours"}
                    title={row.yours ?? "∅"}
                    onclick={(e) => {
                      e.stopPropagation();
                      chooseSide(row, "yours", idx);
                    }}
                  >
                    <span class="mono">{row.yoursDisplay ?? row.yours ?? "∅"}</span>
                  </button>
                </td>
                <td class="side">
                  <button
                    type="button"
                    class="pick"
                    class:chosen={picked?.choice === "theirs"}
                    title={row.theirs ?? "∅"}
                    onclick={(e) => {
                      e.stopPropagation();
                      chooseSide(row, "theirs", idx);
                    }}
                  >
                    <span class="mono">{row.theirsDisplay ?? row.theirs ?? "∅"}</span>
                  </button>
                </td>
                <td class="rescol">
                  {#if editing?.id === row.id}
                    <div class="editrow">
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
                    </div>
                  {:else}
                    <div class="resval">
                      <span class="mono">{picked ? (picked.value ?? "∅") : "—"}</span>
                      <button
                        type="button"
                        class="sm"
                        onclick={(e) => {
                          e.stopPropagation();
                          startEdit(row, idx);
                        }}>Edit</button
                      >
                    </div>
                  {/if}
                </td>
              </tr>
            {/each}
          </tbody>
        </table>
      {/if}

      {#if table.autoMerged.length > 0}
        <details class="auto">
          <summary>Auto-merged cleanly ({table.autoMerged.length} properties)</summary>
          <ul>
            {#each table.autoMerged as item (`${item.fileId}-${item.property}`)}
              <li>
                <b>{item.objectPath}</b> › {item.component} · <code class="mono">{item.property}</code> ({item.side}):
                <code class="mono">{item.display ?? item.value ?? "∅"}</code>
              </li>
            {/each}
          </ul>
        </details>
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
  .scroll {
    flex: 1;
    overflow: auto;
    padding: 8px 10px;
  }
  .tbl {
    width: 100%;
    border-collapse: collapse;
    font-size: 11.5px;
  }
  .tbl th {
    text-align: left;
    padding: 5px 8px;
    font-weight: 600;
    color: var(--muted);
    border-bottom: 1px solid var(--line);
    white-space: nowrap;
  }
  .tbl td {
    padding: 6px 8px;
    border-bottom: 1px solid var(--line);
    vertical-align: top;
  }
  .tbl tr.active td {
    background: var(--hover);
  }
  .obj {
    font-weight: 600;
  }
  .comp {
    font-weight: 400;
    color: var(--muted);
  }
  .pname {
    color: var(--l0);
    font-size: 11px;
  }
  .base {
    color: var(--muted);
    word-break: break-word;
  }
  .pick {
    width: 100%;
    text-align: left;
    padding: 4px 7px;
    border: 1px solid var(--line);
    border-radius: 4px;
    background: var(--panel);
    word-break: break-word;
  }
  .pick:hover {
    border-color: var(--l0);
  }
  .pick.chosen {
    background: var(--sel);
    border-color: var(--l0);
    color: var(--l0);
  }
  .resval,
  .editrow {
    display: flex;
    align-items: center;
    gap: 6px;
    justify-content: space-between;
  }
  .editrow input {
    min-width: 90px;
    height: 22px;
    font-size: 11px;
  }
  .auto {
    margin-top: 12px;
    border: 1px solid var(--line);
    border-radius: 5px;
    padding: 6px 8px;
    font-size: 11.5px;
  }
  .auto summary {
    cursor: default;
    font-weight: 600;
    color: var(--muted);
  }
  .auto ul {
    margin: 6px 0 0;
    padding-left: 18px;
    display: grid;
    gap: 3px;
  }
  .mono {
    font-family: var(--mono);
  }
  .err {
    margin: 10px;
    color: var(--bad-ink);
  }
  .note {
    margin: 10px;
    color: var(--muted);
  }
</style>
