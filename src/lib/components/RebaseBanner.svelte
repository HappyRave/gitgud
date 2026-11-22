<script lang="ts">
  import type { RebaseState } from "$lib/api";

  let {
    rebase,
    busy = false,
    oncontinue,
    onskip,
    onabort,
  }: {
    rebase: RebaseState;
    busy?: boolean;
    oncontinue: () => void;
    onskip: () => void;
    onabort: () => void;
  } = $props();

  const left = $derived(rebase.conflicts.filter((c) => !c.resolved).length);
</script>

{#if rebase.editing}
  <div class="banner" role="region" aria-label="Rebase in progress">
    <span class="ic">⟲</span>
    <span class="what">
      Stopped at step {rebase.step} of {rebase.total} to edit <b>{rebase.subject}</b> · change files and stage them; Continue amends the
      commit
    </span>
    <span class="sp"></span>
    <button class="pri" disabled={busy} title="Amend the commit with what is staged, then apply the next commits" onclick={oncontinue}
      >Continue</button
    >
    <button class="bad" disabled={busy} title="Undoable with Ctrl+Z" onclick={onabort}>Abort, back to before</button>
  </div>
{:else}
<div class="banner" class:done={left === 0} role="region" aria-label="Rebase in progress">
  <span class="ic">⟲</span>
  <span class="what">
    Rebasing <b>{rebase.branch}</b> onto <b>{rebase.onto}</b> · step {rebase.step} of {rebase.total}
    ·
    {left ? `${left} of ${rebase.conflicts.length} ${rebase.conflicts.length === 1 ? "file" : "files"} left` : "resolved"}
  </span>
  <span class="sp"></span>
  <button class:pri={left === 0} disabled={left > 0 || busy} title={left ? "Resolve every file first" : "Apply the next commits"} onclick={oncontinue}
    >Continue</button
  >
  <button disabled={busy} title="Drop this commit and go on" onclick={onskip}>Skip commit</button>
  <button class="bad" disabled={busy} title="Undoable with Ctrl+Z" onclick={onabort}>Abort, back to before</button>
</div>
{/if}

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
