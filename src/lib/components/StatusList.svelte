<script lang="ts">
  import type { Change, Entry, Head, Status } from "$lib/api";

  let { status }: { status: Status } = $props();

  const LETTER: Record<Change, string> = { added: "A", modified: "M", deleted: "D", typeChange: "T" };

  const groups = $derived(
    [
      { title: "Conflicts", rows: status.entries.filter((e) => e.kind === "conflicted"), change: () => "!" },
      { title: "Staged", rows: status.entries.filter((e) => e.index), change: (e: Entry) => LETTER[e.index!] },
      { title: "Changes", rows: status.entries.filter((e) => e.worktree), change: (e: Entry) => LETTER[e.worktree!] },
      { title: "Untracked", rows: status.entries.filter((e) => e.kind === "untracked"), change: () => "?" },
    ].filter((g) => g.rows.length > 0),
  );

  function headText(head: Head) {
    switch (head.kind) {
      case "branch":
        return head.name;
      case "unborn":
        return `${head.name} (no commits yet)`;
      case "detached":
        return `detached at ${head.id}`;
    }
  }
</script>

<section aria-label="Working copy">
  <p class="head" data-testid="head" class:detached={status.head.kind === "detached"}>{headText(status.head)}</p>
  <p class="root">{status.root}</p>

  {#if groups.length === 0}
    <p class="clean">Nothing to commit, working tree clean.</p>
  {/if}

  {#each groups as group (group.title)}
    <h2>{group.title} <span class="count">{group.rows.length}</span></h2>
    <ul aria-label={group.title}>
      {#each group.rows as entry (entry.path)}
        <li><span class="badge">{group.change(entry)}</span>{entry.path}</li>
      {/each}
    </ul>
  {/each}
</section>

<style>
  .head {
    font-weight: 600;
    margin: 0;
  }
  .detached {
    color: #b3261e;
  }
  .root {
    color: #666;
    font-size: 0.85em;
    margin: 0 0 1rem;
  }
  h2 {
    font-size: 0.8rem;
    text-transform: uppercase;
    letter-spacing: 0.04em;
    margin: 1rem 0 0.25rem;
  }
  .count {
    color: #888;
  }
  ul {
    list-style: none;
    margin: 0;
    padding: 0;
    font-family: ui-monospace, monospace;
    font-size: 0.85rem;
  }
  li {
    line-height: 24px;
  }
  .badge {
    display: inline-block;
    width: 1.5em;
    color: #888;
  }
</style>
