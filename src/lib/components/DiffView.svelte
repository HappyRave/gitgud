<script lang="ts">
  import type { FileDiff, Hunk } from "$lib/api";
  import { splitRows } from "$lib/diff/split";
  import ImageDiff, { type ImageSide } from "./ImageDiff.svelte";

  let {
    diff,
    hunkActions = [],
    busy = false,
    focused = null,
    images = null,
    split = false,
  }: {
    /** Side by side instead of unified (Ctrl+D). */
    split?: boolean;
    /** For a binary image: its two versions, shown as an image diff. */
    images?: { before: ImageSide; after: ImageSide } | null;
    diff: FileDiff;
    /** Buttons on each hunk's header row, e.g. "Stage hunk", "Discard hunk". */
    hunkActions?: { label: string; key?: string; run: (hunk: Hunk) => void }[];
    busy?: boolean;
    /** The hunk the keyboard acts on (Alt+↑↓), highlighted and kept in view. */
    focused?: number | null;
  } = $props();

  let table = $state<HTMLElement | null>(null);

  $effect(() => {
    if (focused !== null) {
      table?.querySelectorAll("tbody")[focused]?.scrollIntoView?.({ block: "nearest" });
    }
  });

  const SIGN = { context: " ", added: "+", removed: "-" } as const;

  function header(h: FileDiff["hunks"][number]) {
    const range = (start: number, len: number) => (len === 1 ? `${start}` : `${start},${len}`);
    return `@@ -${range(h.oldStart, h.oldLines)} +${range(h.newStart, h.newLines)} @@`;
  }
</script>

<section class="diff" aria-label="Diff of {diff.path}">
  {#if diff.binary && images}
    <ImageDiff before={images.before} after={images.after} />
  {:else if diff.binary}
    <p class="note">Binary file, no text diff.</p>
  {:else if diff.hunks.length === 0}
    <p class="note">No content changes.</p>
  {:else}
    <table bind:this={table}>
      {#each diff.hunks as hunk, h (h)}
        <tbody class:focused={focused === h}>
          <tr class="hunk">
            <td colspan={split ? 4 : 3}>
              {header(hunk)}
              {#each hunkActions as action (action.label)}
                <button disabled={busy} aria-label="{action.label} {header(hunk)}" onclick={() => action.run(hunk)}
                  >{action.label}{#if action.key && focused === h}<span class="k" aria-hidden="true">{action.key}</span>{/if}</button
                >
              {/each}
            </td>
          </tr>
          {#if split}
            {#each splitRows(hunk) as row, i (i)}
              <tr class="pair">
                <td class="num">{row.left?.old ?? ""}</td>
                <td class="text half {row.left ? row.left.kind : 'none'}">{row.left?.text ?? ""}</td>
                <td class="num">{row.right?.new ?? ""}</td>
                <td class="text half {row.right ? row.right.kind : 'none'}">{row.right?.text ?? ""}</td>
              </tr>
            {/each}
          {:else}
            {#each hunk.lines as line, i (i)}
              <tr class={line.kind}>
                <td class="num">{line.old ?? ""}</td>
                <td class="num">{line.new ?? ""}</td>
                <td class="text"><span class="sign">{SIGN[line.kind]}</span>{line.text}</td>
              </tr>
            {/each}
          {/if}
        </tbody>
      {/each}
    </table>
  {/if}
</section>

<style>
  .diff {
    overflow: auto;
    font: 12px/18px var(--mono);
  }
  table {
    border-collapse: collapse;
    width: 100%;
  }
  td {
    padding: 0 6px;
    white-space: pre;
  }
  .num {
    color: var(--muted);
    opacity: 0.75;
    text-align: right;
    user-select: none;
    width: 1%;
  }
  .hunk td {
    height: 28px;
    color: var(--muted);
    background: var(--hover);
    border-block: 1px solid var(--line);
    padding: 0 10px;
  }
  .focused .hunk td {
    box-shadow: inset 3px 0 0 var(--l0);
    color: var(--fg);
  }
  .hunk button {
    float: right;
    margin-left: 6px;
    height: 20px;
    padding: 0 7px;
    font-size: 11px;
  }
  .added {
    background: var(--add);
  }
  .half {
    width: 50%;
    white-space: pre-wrap;
    overflow-wrap: anywhere;
  }
  .half + .num {
    border-left: 1px solid var(--line);
  }
  .half.none {
    background: repeating-linear-gradient(-45deg, transparent 0 4px, var(--hover) 4px 8px);
  }
  .added .sign {
    color: var(--add-ink);
  }
  .removed {
    background: var(--del);
  }
  .removed .sign {
    color: var(--bad-ink);
  }
  .sign {
    user-select: none;
    color: var(--muted);
    margin-right: 6px;
  }
  .note {
    color: var(--muted);
    font-family: var(--body);
    padding: 10px;
    margin: 0;
  }
</style>
