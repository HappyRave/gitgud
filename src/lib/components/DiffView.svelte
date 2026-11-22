<script lang="ts">
  import { hunkHeader, type DiffLine, type FileDiff, type Hunk, type LineSelect } from "$lib/api";
  import { detectLanguage, highlightLine, mergeInline, type StyledSegment } from "$lib/diff/highlight";
  import { pairHunkLines, type InlineSpan } from "$lib/diff/intraline";
  import { splitRows } from "$lib/diff/split";
  import ImageDiff, { type ImageSide } from "./ImageDiff.svelte";

  export interface LineAction {
    label: string | ((count: number) => string);
    run: (select: LineSelect) => void;
  }

  let {
    diff,
    hunkActions = [],
    lineActions = [],
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
    /** Actions rendered when changed lines inside a hunk are clicked/selected. */
    lineActions?: LineAction[];
    busy?: boolean;
    /** The hunk the keyboard acts on (Alt+↑↓), highlighted and kept in view. */
    focused?: number | null;
  } = $props();

  let table = $state<HTMLElement | null>(null);
  let selectedHunk = $state<number | null>(null);
  let selectedLines = $state<Set<number>>(new Set());
  let anchorLine = $state<number | null>(null);

  const lang = $derived(detectLanguage(diff.path));
  const intralineByHunk = $derived(diff.hunks.map((h) => pairHunkLines(h)));

  $effect(() => {
    void diff;
    selectedHunk = null;
    selectedLines = new Set();
    anchorLine = null;
  });

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

  function clearSelection() {
    selectedHunk = null;
    selectedLines = new Set();
    anchorLine = null;
  }

  function onLineClick(hIdx: number, lineIdx: number, line: DiffLine, event: MouseEvent) {
    if (lineActions.length === 0 || line.kind === "context") {
      return;
    }
    if (event.shiftKey) {
      event.preventDefault();
    }
    const hunk = diff.hunks[hIdx];
    const changeable = (idx: number) => hunk.lines[idx]?.kind !== "context";
    if (selectedHunk !== hIdx) {
      selectedHunk = hIdx;
      selectedLines = new Set([lineIdx]);
      anchorLine = lineIdx;
      return;
    }
    const next = new Set(selectedLines);
    if (event.shiftKey && anchorLine !== null) {
      const lo = Math.min(anchorLine, lineIdx);
      const hi = Math.max(anchorLine, lineIdx);
      for (let i = lo; i <= hi; i++) {
        if (changeable(i)) {
          next.add(i);
        }
      }
    } else if (next.has(lineIdx)) {
      next.delete(lineIdx);
      anchorLine = lineIdx;
    } else {
      next.add(lineIdx);
      anchorLine = lineIdx;
    }
    if (next.size === 0) {
      clearSelection();
    } else {
      selectedLines = next;
    }
  }

  function runLineAction(hunk: Hunk, action: LineAction) {
    const lines = [...selectedLines].sort((a, b) => a - b);
    if (lines.length === 0) {
      return;
    }
    action.run({ hunk: hunkHeader(hunk), lines });
    clearSelection();
  }

  function actionLabel(label: LineAction["label"], count: number) {
    return typeof label === "function" ? label(count) : `${label} (${count})`;
  }

  function styledText(text: string, inline?: InlineSpan[]): StyledSegment[] {
    const tokens = highlightLine(text, lang);
    return mergeInline(tokens, inline);
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
        {@const pairs = intralineByHunk[h]}
        {@const hasSel = selectedHunk === h && selectedLines.size > 0 && lineActions.length > 0}
        <tbody class:focused={focused === h}>
          <tr class="hunk">
            <td colspan={split ? 4 : 3}>
              {header(hunk)}
              {#each hunkActions as action (action.label)}
                <button disabled={busy} aria-label="{action.label} {header(hunk)}" onclick={() => action.run(hunk)}
                  >{action.label}{#if action.key && focused === h}<span class="k" aria-hidden="true">{action.key}</span>{/if}</button
                >
              {/each}
              {#if hasSel}
                <button type="button" class="clear-sel" disabled={busy} onclick={clearSelection} title="Clear line selection"
                  >Clear ({selectedLines.size})</button
                >
                {#each lineActions as la, ai (ai)}
                  <button type="button" class="line-act" disabled={busy} onclick={() => runLineAction(hunk, la)}
                    >{actionLabel(la.label, selectedLines.size)}</button
                  >
                {/each}
              {/if}
            </td>
          </tr>
          {#if split}
            {#each splitRows(hunk) as row, i (i)}
              {@const leftSel = row.leftIndex !== null && selectedHunk === h && selectedLines.has(row.leftIndex)}
              {@const rightSel = row.rightIndex !== null && selectedHunk === h && selectedLines.has(row.rightIndex)}
              <tr class="pair">
                <td
                  class="num"
                  class:selectable={lineActions.length > 0 && row.left !== null && row.left.kind !== "context"}
                  class:linesel={leftSel}
                  onclick={(e) => row.left && row.leftIndex !== null && onLineClick(h, row.leftIndex, row.left, e)}
                  >{row.left?.old ?? ""}</td
                >
                <td
                  class="text half {row.left ? row.left.kind : 'none'}"
                  class:selectable={lineActions.length > 0 && row.left !== null && row.left.kind !== "context"}
                  class:linesel={leftSel}
                  onclick={(e) => row.left && row.leftIndex !== null && onLineClick(h, row.leftIndex, row.left, e)}
                  >{#if row.left}{#each styledText(row.left.text, row.leftIndex !== null ? pairs.get(row.leftIndex) : undefined) as seg, si (si)}<span
                        class="tok-{seg.kind}"
                        class:chg={seg.changed}>{seg.text}</span
                      >{/each}{/if}</td
                >
                <td
                  class="num"
                  class:selectable={lineActions.length > 0 && row.right !== null && row.right.kind !== "context"}
                  class:linesel={rightSel}
                  onclick={(e) => row.right && row.rightIndex !== null && onLineClick(h, row.rightIndex, row.right, e)}
                  >{row.right?.new ?? ""}</td
                >
                <td
                  class="text half {row.right ? row.right.kind : 'none'}"
                  class:selectable={lineActions.length > 0 && row.right !== null && row.right.kind !== "context"}
                  class:linesel={rightSel}
                  onclick={(e) => row.right && row.rightIndex !== null && onLineClick(h, row.rightIndex, row.right, e)}
                  >{#if row.right}{#each styledText(row.right.text, row.rightIndex !== null ? pairs.get(row.rightIndex) : undefined) as seg, si (si)}<span
                        class="tok-{seg.kind}"
                        class:chg={seg.changed}>{seg.text}</span
                      >{/each}{/if}</td
                >
              </tr>
            {/each}
          {:else}
            {#each hunk.lines as line, i (i)}
              {@const isSel = selectedHunk === h && selectedLines.has(i)}
              <tr
                class={line.kind}
                class:selectable={lineActions.length > 0 && line.kind !== "context"}
                class:linesel={isSel}
                aria-selected={lineActions.length > 0 && line.kind !== "context" ? isSel : undefined}
                onclick={(e) => onLineClick(h, i, line, e)}
              >
                <td class="num">{line.old ?? ""}</td>
                <td class="num">{line.new ?? ""}</td>
                <td class="text"
                  ><span class="sign">{SIGN[line.kind]}</span>{#each styledText(line.text, pairs.get(i)) as seg, si (si)}<span
                      class="tok-{seg.kind}"
                      class:chg={seg.changed}>{seg.text}</span
                    >{/each}</td
                >
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
  .hunk .line-act {
    background: var(--sel);
    border-color: var(--l0);
  }
  .added {
    background: var(--add);
  }
  .added .chg {
    background: var(--add-hi);
    border-radius: 2px;
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
  .removed .chg {
    background: var(--del-hi);
    border-radius: 2px;
  }
  .removed .sign {
    color: var(--bad-ink);
  }
  .selectable {
    cursor: pointer;
  }
  tr.linesel > td,
  td.linesel {
    box-shadow: inset 3px 0 0 var(--l0);
    filter: brightness(0.96);
  }
  .tok-kw {
    color: var(--syn-kw);
  }
  .tok-str {
    color: var(--syn-str);
  }
  .tok-num {
    color: var(--syn-num);
  }
  .tok-com {
    color: var(--syn-com);
    font-style: italic;
  }
  .tok-type {
    color: var(--syn-type);
  }
  .tok-fn {
    color: var(--syn-fn);
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
