<script lang="ts">
  import type { SequencerState } from "$lib/api";

  let {
    sequencer,
    busy = false,
    oncontinue,
    onabort,
  }: {
    sequencer: SequencerState;
    busy?: boolean;
    oncontinue: () => void;
    onabort: () => void;
  } = $props();

  const left = $derived(sequencer.conflicts.filter((c) => !c.resolved).length);
  const label = $derived(sequencer.kind === "cherryPick" ? "Cherry-picking" : "Reverting");
</script>

<div class="banner" class:done={left === 0} role="region" aria-label="{label} in progress">
  <span class="ic">⇄</span>
  <span class="what">
    {label} <b>{sequencer.shortId}</b> "{sequencer.subject}" ·
    {left ? `${left} of ${sequencer.conflicts.length} ${sequencer.conflicts.length === 1 ? "file" : "files"} left` : "resolved"}
  </span>
  <span class="sp"></span>
  <button class:pri={left === 0} disabled={left > 0 || busy} onclick={oncontinue}>Continue</button>
  <button class="bad" disabled={busy} title="Undoable with Ctrl+Z" onclick={onabort}>Abort, back to before</button>
</div>

<style>
  .banner {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 5px 12px;
    background: var(--warn);
    color: var(--warn-ink);
    border-bottom: 1px solid var(--line);
    font-weight: 500;
    white-space: nowrap;
  }
  .banner.done {
    background: var(--add);
    color: var(--add-ink);
  }
  .what {
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .sp {
    flex: 1;
  }
  button {
    height: 22px;
  }
</style>
