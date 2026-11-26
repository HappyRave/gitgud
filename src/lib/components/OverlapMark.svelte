<script lang="ts">
  import type { OverlapFile } from "$lib/api";
  import { ago, initials, personColor, RISK_LABEL } from "$lib/overlap/radar";

  let { file }: { file: OverlapFile } = $props();

  const title = $derived(
    `Also changed on ${file.on} by ${file.authors.join(", ")} (${file.commits} commit${file.commits === 1 ? "" : "s"}, newest ${ago(file.newest)}). ` +
      {
        wontMerge: "Git can't merge it line by line: pull before you go further.",
        mayConflict: "Both sides changed the same lines: pulling now keeps the conflict small.",
        merges: "The changes are in different places: git will merge them.",
      }[file.risk],
  );
</script>

<span class="mark {file.risk}" {title}>
  {#each file.authors.slice(0, 3) as name (name)}
    <span class="who" style:background={personColor(name)}>{initials(name)}</span>
  {/each}
  {RISK_LABEL[file.risk]}{#if file.risk !== "merges"}&nbsp;· {ago(file.newest).replace(" ago", "")}{/if}
</span>

<style>
  .mark {
    flex: none;
    display: inline-flex;
    align-items: center;
    gap: 3px;
    font: 600 10px var(--mono);
    padding: 1px 6px 1px 2px;
    border-radius: 9px;
    white-space: nowrap;
    background: var(--hover);
    color: var(--muted);
  }
  .mark.wontMerge {
    background: var(--bad);
    color: var(--bad-ink);
  }
  .mark.mayConflict {
    background: var(--warn);
    color: var(--warn-ink);
  }
  .who {
    display: inline-grid;
    place-items: center;
    min-width: 14px;
    height: 14px;
    padding: 0 2px;
    border-radius: 7px;
    font: 700 7.5px var(--mono);
    color: #10141b;
  }
</style>
