<script lang="ts">
  import { blameFile, fileHistory, type BlameLine, type FileCommit } from "$lib/api";

  let {
    repoId,
    path,
    initialMode = "history",
    rev = null,
    onselectcommit,
    onclose,
  }: {
    repoId: number;
    path: string;
    initialMode?: "history" | "blame";
    rev?: string | null;
    onselectcommit?: (commitId: string) => void;
    onclose: () => void;
  } = $props();

  const CHANGE_BADGE: Record<string, string> = {
    added: "A",
    modified: "M",
    deleted: "D",
    renamed: "R",
    typeChange: "T",
  };

  let mode = $state<"history" | "blame">("history");
  let history = $state<FileCommit[] | null>(null);
  let blame = $state<BlameLine[] | null>(null);
  let error = $state<string | null>(null);

  $effect(() => {
    mode = initialMode;
  });

  $effect(() => {
    const id = repoId;
    const p = path;
    const m = mode;
    const r = rev;
    let stale = false;
    error = null;

    if (m === "history") {
      history = null;
      fileHistory(id, p).then(
        (items) => {
          if (!stale) {
            history = items;
          }
        },
        (err) => {
          if (!stale) {
            error = String(err);
          }
        },
      );
    } else {
      blame = null;
      blameFile(id, p, r).then(
        (lines) => {
          if (!stale) {
            blame = lines;
          }
        },
        (err) => {
          if (!stale) {
            error = String(err);
          }
        },
      );
    }

    return () => {
      stale = true;
    };
  });

  function shortDate(secs: number): string {
    if (!secs) {
      return "";
    }
    return new Date(secs * 1000).toLocaleDateString(undefined, { month: "short", day: "numeric", year: "2-digit" });
  }

  function laneVar(commit: string): string {
    let sum = 0;
    for (let i = 0; i < Math.min(commit.length, 8); i++) {
      sum += commit.charCodeAt(i);
    }
    return `var(--l${sum % 5})`;
  }
</script>

<section class="fhb" aria-label="File history and blame for {path}">
  <div class="ph">
    <span class="title mono" title={path}>{path}</span>
    <span class="r actions">
      <button
        type="button"
        class="tab"
        class:on={mode === "history"}
        aria-pressed={mode === "history"}
        onclick={() => (mode = "history")}>History</button
      >
      <button
        type="button"
        class="tab"
        class:on={mode === "blame"}
        aria-pressed={mode === "blame"}
        onclick={() => (mode = "blame")}>Blame</button
      >
      <button type="button" class="tab" aria-label="Close file inspector" onclick={onclose}>✕ Close</button>
    </span>
  </div>

  {#if error}
    <p class="err" role="alert">{error}</p>
  {:else if mode === "history"}
    {#if history === null}
      <p class="note">Loading history…</p>
    {:else if history.length === 0}
      <p class="note">No commits recorded for this file.</p>
    {:else}
      <div class="scroll">
        <ul class="hist" aria-label="File commits">
          {#each history as item (item.id)}
            <li>
              <button type="button" class="hrow" onclick={() => onselectcommit?.(item.id)}>
                <span class="badge c-{CHANGE_BADGE[item.change] ?? 'M'}">{CHANGE_BADGE[item.change] ?? "M"}</span>
                <span class="sha mono">{item.shortId}</span>
                <span class="subj">{item.subject}</span>
                <span class="meta">{item.author} · {shortDate(item.time)}</span>
              </button>
            </li>
          {/each}
        </ul>
      </div>
    {/if}
  {:else if blame === null}
    <p class="note">Loading blame…</p>
  {:else if blame.length === 0}
    <p class="note">No blame annotations available.</p>
  {:else}
    <div class="scroll">
      <table class="btbl" aria-label="Blame lines">
        <tbody>
          {#each blame as line, i (line.lineNo)}
            {@const same = i > 0 && blame[i - 1].commit === line.commit}
            <tr class:block={!same}>
              <td class="gutter" style:--stripe={laneVar(line.commit)}>
                {#if !same}
                  <button
                    type="button"
                    class="bare cbtn"
                    title="{line.shortCommit} · {line.author} · {line.summary}"
                    onclick={() => onselectcommit?.(line.commit)}
                  >
                    <span class="sha mono">{line.shortCommit}</span>
                    <span class="au">{line.author}</span>
                    <span class="dt">{shortDate(line.time)}</span>
                  </button>
                {/if}
              </td>
              <td class="ln mono">{line.lineNo}</td>
              <td class="code mono">{line.text}</td>
            </tr>
          {/each}
        </tbody>
      </table>
    </div>
  {/if}
</section>

<style>
  .fhb {
    display: flex;
    flex-direction: column;
    min-height: 0;
    height: 100%;
  }
  .title {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    text-transform: none;
    letter-spacing: 0;
  }
  .actions {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    flex: none;
  }
  .tab {
    height: 20px;
    padding: 0 7px;
    font-size: 11px;
  }
  .tab.on {
    background: var(--sel);
    border-color: var(--l0);
    color: var(--l0);
  }
  .mono {
    font-family: var(--mono);
  }
  .scroll {
    flex: 1;
    min-height: 0;
    overflow: auto;
  }
  .hist {
    list-style: none;
    margin: 0;
    padding: 2px 0;
  }
  .hrow {
    all: unset;
    box-sizing: border-box;
    width: 100%;
    display: grid;
    grid-template-columns: 16px 58px minmax(0, 1fr) auto;
    align-items: center;
    gap: 8px;
    padding: 4px 10px;
    cursor: default;
  }
  .hrow:hover {
    background: var(--hover);
  }
  .hrow:focus-visible {
    outline: 2px solid var(--l0);
    outline-offset: -2px;
  }
  .badge {
    all: unset;
    font: 600 10.5px var(--mono);
    width: 16px;
    text-align: center;
    color: var(--l4);
  }
  .c-A {
    color: var(--l2);
  }
  .c-D {
    color: var(--bad-ink);
  }
  .c-R {
    color: var(--l0);
  }
  .sha {
    font-size: 11px;
    color: var(--l0);
  }
  .subj {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .meta {
    font-size: 11px;
    color: var(--muted);
    white-space: nowrap;
  }
  .btbl {
    width: 100%;
    border-collapse: collapse;
    font-size: 11.5px;
    line-height: 19px;
  }
  .btbl tr.block td {
    border-top: 1px solid var(--line);
  }
  .btbl tr:first-child td {
    border-top: 0;
  }
  .gutter {
    width: 190px;
    max-width: 190px;
    padding: 0 6px;
    border-left: 3px solid var(--stripe);
    vertical-align: top;
    white-space: nowrap;
    overflow: hidden;
  }
  .cbtn {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    max-width: 100%;
    cursor: default;
  }
  .cbtn:hover .sha {
    text-decoration: underline;
  }
  .au {
    overflow: hidden;
    text-overflow: ellipsis;
    color: var(--fg);
  }
  .dt {
    color: var(--muted);
    font-size: 10.5px;
  }
  .ln {
    width: 36px;
    padding: 0 8px 0 4px;
    text-align: right;
    color: var(--muted);
    user-select: none;
    vertical-align: top;
  }
  .code {
    padding: 0 8px;
    white-space: pre;
  }
  .err {
    margin: 8px 10px;
    color: var(--bad-ink);
  }
  .note {
    margin: 10px;
    color: var(--muted);
  }
</style>
