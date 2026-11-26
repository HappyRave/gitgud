<script lang="ts">
  import { onMount } from "svelte";
  import type { AddRepoEntry, ScanResult } from "$lib/api";
  import Dialog from "./Dialog.svelte";

  let {
    scan,
    rescanOf = null,
    busy = false,
    onadd,
    oncancel,
  }: {
    scan: ScanResult;
    /** The Library folder being rescanned, for the title; null for a plain scan. */
    rescanOf?: string | null;
    busy?: boolean;
    onadd: (entries: AddRepoEntry[], removeIds: string[]) => void;
    oncancel: () => void;
  } = $props();

  const gone = $derived(scan.gone ?? []);
  let keep = $state<Record<string, boolean>>({});
  const removeIds = $derived(gone.filter((g) => !keep[g.id]).map((g) => g.id));

  let checked = $state<Record<string, boolean>>({});
  let folders = $state<Record<string, string>>({});
  let addButton = $state<HTMLButtonElement | null>(null);

  onMount(() => {
    const nextChecked: Record<string, boolean> = {};
    const nextFolders: Record<string, string> = {};
    for (const g of scan.groups) {
      const freshCount = g.repos.filter((r) => !r.alreadyInLibrary).length;
      nextChecked[g.dir] = freshCount > 0;
      nextFolders[g.dir] = g.suggestedFolder;
    }
    checked = nextChecked;
    folders = nextFolders;
    requestAnimationFrame(() => addButton?.focus());
  });

  const existingCount = $derived(
    scan.groups.reduce((acc, g) => acc + g.repos.filter((r) => r.alreadyInLibrary).length, 0),
  );

  const selectedEntries = $derived.by(() => {
    const out: AddRepoEntry[] = [];
    for (const g of scan.groups) {
      if (!checked[g.dir]) {
        continue;
      }
      const folderPath = (folders[g.dir] ?? g.suggestedFolder).trim() || null;
      for (const r of g.repos) {
        if (!r.alreadyInLibrary) {
          out.push({ path: r.path, folderPath, name: r.name });
        }
      }
    }
    return out;
  });
</script>

<Dialog title={rescanOf ? `Rescan ${rescanOf}` : `Add repositories · scan ${scan.root}`} width={620} {oncancel}>
  <div class="hdline">
    <code>{scan.root}</code>
    <span class="ab">{scan.totalFound} found in {(scan.elapsedMs / 1000).toFixed(1)} s</span>
  </div>

  {#if scan.groups.length === 0}
    <p class="ab">{rescanOf ? "Nothing new on disk here." : "No Git repositories were found inside this folder."}</p>
  {:else}
    <div class="groups">
      {#each scan.groups as group (group.dir)}
        {@const fresh = group.repos.filter((r) => !r.alreadyInLibrary).length}
        <div class="row">
          <input
            type="checkbox"
            aria-label="Include {group.dir}"
            disabled={fresh === 0}
            checked={!!checked[group.dir]}
            onchange={(e) => (checked = { ...checked, [group.dir]: (e.currentTarget as HTMLInputElement).checked })}
          />
          <div class="info">
            <b>{group.dir}</b>
            <span class="ab">
              {group.repos.length} {group.repos.length === 1 ? "repo" : "repos"}
              {#if fresh < group.repos.length}
                · {fresh} new
              {/if}
            </span>
          </div>
          <label class="fld">
            <span class="ab">→</span>
            <input
              type="text"
              aria-label="Folder for {group.dir}"
              disabled={!checked[group.dir]}
              value={folders[group.dir] ?? group.suggestedFolder}
              oninput={(e) => (folders = { ...folders, [group.dir]: (e.currentTarget as HTMLInputElement).value })}
            />
          </label>
        </div>
      {/each}
    </div>
  {/if}

  {#if existingCount > 0}
    <p class="ab">{existingCount} of these {existingCount === 1 ? "is" : "are"} already in gitgud and stay where they are.</p>
  {/if}

  {#if gone.length > 0}
    <h3>Gone from disk · {gone.length}</h3>
    <div class="groups" role="list" aria-label="Gone from disk">
      {#each gone as repo (repo.id)}
        <label class="row gone" role="listitem">
          <input
            type="checkbox"
            aria-label="Remove {repo.name}"
            checked={!keep[repo.id]}
            onchange={(e) => (keep = { ...keep, [repo.id]: !(e.currentTarget as HTMLInputElement).checked })}
          />
          <b>{repo.name}</b>
          <span class="ab path">{repo.path}</span>
        </label>
      {/each}
    </div>
    <p class="ab">Removing only takes them out of gitgud (Ctrl+Z brings them back); nothing on disk changes.</p>
  {/if}

  {#snippet footer()}
    <span class="ab">Folders are suggested from the disk layout; change any before adding.</span>
    <span class="sp"></span>
    <button onclick={oncancel}>Cancel <span class="k">Esc</span></button>
    <button
      class="pri"
      bind:this={addButton}
      disabled={busy || selectedEntries.length + removeIds.length === 0}
      onclick={() => onadd(selectedEntries, removeIds)}
    >
      {#if removeIds.length === 0}
        Add {selectedEntries.length} {selectedEntries.length === 1 ? "repo" : "repos"}
      {:else if selectedEntries.length === 0}
        Remove {removeIds.length} {removeIds.length === 1 ? "repo" : "repos"}
      {:else}
        Add {selectedEntries.length} · remove {removeIds.length}
      {/if}
    </button>
  {/snippet}
</Dialog>

<style>
  .hdline {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 6px 8px;
    border: 1px solid var(--line);
    border-radius: 6px;
    background: var(--bg);
  }
  .groups {
    max-height: 46vh;
    overflow-y: auto;
    display: grid;
    gap: 4px;
  }
  .row {
    display: grid;
    grid-template-columns: auto minmax(0, 1fr) 210px;
    gap: 8px;
    align-items: center;
    padding: 6px 8px;
    border: 1px solid var(--line);
    border-radius: 6px;
    background: var(--bg);
  }
  .info {
    display: flex;
    gap: 6px;
    align-items: baseline;
    min-width: 0;
  }
  .info b {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .fld {
    display: flex;
    gap: 4px;
    align-items: center;
  }
  .fld input {
    width: 100%;
    font: 12px var(--body);
    padding: 3px 6px;
    border: 1px solid var(--line);
    border-radius: 4px;
    background: var(--panel);
    color: var(--fg);
  }
  .row.gone {
    grid-template-columns: auto auto minmax(0, 1fr);
    cursor: pointer;
  }
  .path {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  h3 {
    margin: 4px 0 0;
    font: 600 11px var(--body);
    text-transform: uppercase;
    letter-spacing: 0.04em;
    color: var(--muted);
  }
  p {
    margin: 0;
  }
  .sp {
    flex: 1;
  }
</style>
