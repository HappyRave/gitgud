<script lang="ts">
  import type { OverlapFile } from "$lib/api";
  import { advice, headline, initials, personColor } from "$lib/overlap/radar";

  let {
    files,
    busy = false,
    onpull,
    oncommitpull,
    onmerge,
    onsee,
    ondismiss,
  }: {
    /** Won't-merge overlaps not announced yet, newest first; the first one is told. */
    files: OverlapFile[];
    busy?: boolean;
    onpull: () => void;
    oncommitpull: () => void;
    /** For a file changed on the main branch, which a pull won't bring. */
    onmerge: (branch: string) => void;
    onsee: (file: OverlapFile) => void;
    ondismiss: () => void;
  } = $props();

  const file = $derived(files[0]);
</script>

<div class="headsup" role="alert" aria-label="Overlap heads-up">
  <div class="h">
    {#each file.authors.slice(0, 3) as name (name)}
      <span class="who" style:background={personColor(name)} title={name}>{initials(name)}</span>
    {/each}
    <b>{headline(file)}</b>
  </div>
  <p>{advice(file)}</p>
  {#if files.length > 1}
    <p class="more">and {files.length - 1} more {files.length === 2 ? "file" : "files"} that won't merge</p>
  {/if}
  <div class="acts">
    {#if file.trunk}
      <button class="pri" disabled={busy} onclick={() => onmerge(file.on)}>Merge {file.on}…</button>
    {:else if file.dirty}
      <button class="pri" disabled={busy} onclick={oncommitpull}>Commit, then pull</button>
    {:else}
      <button class="pri" disabled={busy} onclick={onpull}>Pull now</button>
    {/if}
    <button onclick={() => onsee(file)}>See their change</button>
    <button class="sm" onclick={ondismiss}>Not now</button>
  </div>
</div>

<style>
  .headsup {
    /* Over the bottom of the history, clear of the working copy and its commit box. */
    position: absolute;
    right: calc(var(--insp-w) + 19px);
    bottom: 12px;
    z-index: 30;
    width: min(440px, calc(100% - var(--insp-w) - 40px));
    display: grid;
    gap: 6px;
    padding: 10px 12px;
    border-radius: 6px;
    border: 1px solid var(--warn-ink);
    background: var(--panel);
    box-shadow: 0 8px 28px #0008;
    font-size: 12.5px;
  }
  .h {
    display: flex;
    gap: 6px;
    align-items: center;
  }
  .h b {
    overflow-wrap: anywhere;
  }
  p {
    margin: 0;
    color: var(--muted);
  }
  .more {
    font-size: 11.5px;
  }
  .acts {
    display: flex;
    gap: 6px;
    flex-wrap: wrap;
  }
  .who {
    flex: none;
    display: inline-grid;
    place-items: center;
    min-width: 18px;
    height: 18px;
    padding: 0 3px;
    border-radius: 9px;
    font: 700 8.5px var(--mono);
    color: #10141b;
  }
</style>
