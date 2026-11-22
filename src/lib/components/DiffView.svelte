<script lang="ts">
  import type { FileDiff, Hunk } from "$lib/api";

  let {
    diff,
    hunkActions = [],
    busy = false,
  }: {
    diff: FileDiff;
    /** Buttons on each hunk's header row, e.g. "Stage hunk", "Discard hunk". */
    hunkActions?: { label: string; run: (hunk: Hunk) => void }[];
    busy?: boolean;
  } = $props();

  const SIGN = { context: " ", added: "+", removed: "-" } as const;

  function header(h: FileDiff["hunks"][number]) {
    const range = (start: number, len: number) => (len === 1 ? `${start}` : `${start},${len}`);
    return `@@ -${range(h.oldStart, h.oldLines)} +${range(h.newStart, h.newLines)} @@`;
  }
</script>

<section class="diff" aria-label="Diff of {diff.path}">
  {#if diff.binary}
    <p class="note">Binary file, no text diff.</p>
  {:else if diff.hunks.length === 0}
    <p class="note">No content changes.</p>
  {:else}
    <table>
      {#each diff.hunks as hunk, h (h)}
        <tbody>
          <tr class="hunk">
            <td colspan="3">
              {header(hunk)}
              {#each hunkActions as action (action.label)}
                <button disabled={busy} aria-label="{action.label} {header(hunk)}" onclick={() => action.run(hunk)}
                  >{action.label}</button
                >
              {/each}
            </td>
          </tr>
          {#each hunk.lines as line, i (i)}
            <tr class={line.kind}>
              <td class="num">{line.old ?? ""}</td>
              <td class="num">{line.new ?? ""}</td>
              <td class="text"><span class="sign">{SIGN[line.kind]}</span>{line.text}</td>
            </tr>
          {/each}
        </tbody>
      {/each}
    </table>
  {/if}
</section>

<style>
  .diff {
    overflow: auto;
    font-family: ui-monospace, monospace;
    font-size: 0.8rem;
  }
  table {
    border-collapse: collapse;
    width: 100%;
  }
  td {
    padding: 0 0.4rem;
    white-space: pre;
    line-height: 18px;
  }
  .num {
    color: #999;
    text-align: right;
    user-select: none;
    width: 1%;
  }
  .hunk td {
    color: #6a6a9a;
    background: #f3f3fb;
  }
  .hunk button {
    float: right;
    margin-left: 0.3rem;
    font: inherit;
    font-family: system-ui, sans-serif;
    font-size: 0.75rem;
    padding: 0 0.4rem;
  }
  .added {
    background: #e6f6e8;
  }
  .removed {
    background: #fbe9e9;
  }
  .sign {
    user-select: none;
    color: #888;
    margin-right: 0.3rem;
  }
  .note {
    color: #888;
    font-family: system-ui, sans-serif;
  }
</style>
