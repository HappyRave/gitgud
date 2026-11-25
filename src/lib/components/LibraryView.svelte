<script lang="ts">
  import { tick } from "svelte";
  import { SvelteSet } from "svelte/reactivity";
  import {
    gitInfo,
    libraryCreateFolder,
    libraryDeleteFolder,
    libraryMoveFolder,
    libraryMoveRepo,
    libraryPinRepo,
    libraryRedo,
    libraryRemoveRepo,
    libraryRenameFolder,
    libraryUndo,
    type FetchManyProgress,
    type LibrarySnapshot,
  } from "$lib/api";
  import ContextMenu, { type MenuItem } from "$lib/components/ContextMenu.svelte";
  import Splitter from "$lib/components/Splitter.svelte";
  import UnityIcon from "$lib/components/UnityIcon.svelte";
  import { inTextField, opLogKey } from "$lib/keys";
  import { prefs, setPaneSize } from "$lib/prefs.svelte";
  import {
    buildFolderTree,
    changesTitle,
    filterAndGroupRepos,
    pathKey,
    smartCounts,
    type LibrarySelection,
    type SmartFilter,
  } from "$lib/library/filter";
  import { age } from "$lib/time";

  let {
    active = true,
    suspended = false,
    library,
    openPaths,
    git,
    fetchingMany = null,
    onopen,
    onadd,
    onscan,
    onclone,
    onlocate,
    onfetchvisible,
    onlibrarychange,
    onrefreshsummaries,
    onerror,
  }: {
    active?: boolean;
    suspended?: boolean;
    library: LibrarySnapshot;
    openPaths: ReadonlySet<string>;
    git: ReturnType<typeof gitInfo>;
    fetchingMany?: FetchManyProgress | null;
    onopen: (path: string) => void;
    onadd: (folderId?: string | null) => void;
    onscan: () => void;
    onclone: () => void;
    onlocate: (id: string) => void;
    onfetchvisible: (paths: string[]) => void;
    onlibrarychange: (snap: LibrarySnapshot) => void;
    onrefreshsummaries: (paths: string[]) => void;
    onerror: (msg: string) => void;
  } = $props();

  let selection = $state<LibrarySelection>({ kind: "smart", filter: "all" });
  let query = $state("");
  let filterInput = $state<HTMLInputElement | null>(null);
  let collapsedFolders = new SvelteSet<string>();
  let expandedGroups = new SvelteSet<string>();
  let selectedRepoId = $state<string | null>(null);
  let creatingFolder = $state<{ parentId: string | null; value: string } | null>(null);
  let renamingFolder = $state<{ id: string; value: string } | null>(null);
  let dragItem = $state<{ kind: "repo" | "folder"; id: string } | null>(null);
  let dropTarget = $state<string | null>(null);
  let menu = $state<{ x: number; y: number; items: MenuItem[] } | null>(null);
  const refreshedOnce = new Set<string>();

  const counts = $derived(smartCounts(library.repos));
  const folderNodes = $derived(buildFolderTree(library.folders, library.repos, collapsedFolders));
  const grouped = $derived(filterAndGroupRepos(library.folders, library.repos, selection, query, expandedGroups));

  const SMART_ITEMS: { id: SmartFilter; label: string; icon: string }[] = [
    { id: "uncommitted", label: "Uncommitted", icon: "●" },
    { id: "notPushed", label: "Not pushed", icon: "↑" },
    { id: "locks", label: "Locks I hold", icon: "🔒" },
    { id: "unity", label: "Unity projects", icon: "" },
    { id: "missing", label: "Missing on disk", icon: "⚠" },
  ];

  $effect(() => {
    if (grouped.flat.length === 0) {
      selectedRepoId = null;
    } else if (!selectedRepoId || !grouped.flat.some((r) => r.id === selectedRepoId)) {
      selectedRepoId = grouped.flat[0].id;
    }
  });

  $effect(() => {
    if (!active) {
      return;
    }
    const stale = grouped.flat
      .filter((r) => !r.cached && !refreshedOnce.has(pathKey(r.path)))
      .slice(0, 30)
      .map((r) => r.path);
    if (stale.length > 0) {
      for (const p of stale) {
        refreshedOnce.add(pathKey(p));
      }
      onrefreshsummaries(stale);
    }
  });

  async function mutate(run: () => Promise<LibrarySnapshot>) {
    try {
      onlibrarychange(await run());
    } catch (err) {
      onerror(String(err));
    }
  }

  function startNewFolder(parentId: string | null = null) {
    creatingFolder = { parentId, value: "" };
  }

  async function commitNewFolder() {
    const draft = creatingFolder;
    creatingFolder = null;
    if (!draft || !draft.value.trim()) {
      return;
    }
    await mutate(() => libraryCreateFolder(draft.value.trim(), draft.parentId));
  }

  async function commitRenameFolder() {
    const draft = renamingFolder;
    renamingFolder = null;
    if (!draft || !draft.value.trim()) {
      return;
    }
    await mutate(() => libraryRenameFolder(draft.id, draft.value.trim()));
  }

  function openFolderMenu(event: MouseEvent, folderId: string, folderName: string) {
    event.preventDefault();
    menu = {
      x: event.clientX,
      y: event.clientY,
      items: [
        { label: "New subfolder…", run: () => startNewFolder(folderId) },
        { label: "Rename", hint: "F2", run: () => (renamingFolder = { id: folderId, value: folderName }) },
        { label: "Delete folder", run: () => mutate(() => libraryDeleteFolder(folderId)) },
      ],
    };
  }

  function openRepoMenu(event: MouseEvent, repoId: string, path: string, pinned: boolean, missing: boolean) {
    event.preventDefault();
    const items: MenuItem[] = [];
    if (!missing) {
      items.push({ label: "Open in tab", hint: "Enter", run: () => onopen(path) });
    } else {
      items.push({ label: "Locate…", run: () => onlocate(repoId) });
    }
    items.push({ label: pinned ? "Unpin" : "Pin to top", run: () => mutate(() => libraryPinRepo(repoId, !pinned)) });
    items.push({ label: "Remove from gitgud", hint: "Del", run: () => mutate(() => libraryRemoveRepo(repoId)) });
    menu = { x: event.clientX, y: event.clientY, items };
  }

  function onKeydown(event: KeyboardEvent) {
    if (!active || suspended || creatingFolder || renamingFolder) {
      return;
    }
    const op = opLogKey(event);
    if (op) {
      event.preventDefault();
      mutate(() => (op === "undo" ? libraryUndo() : libraryRedo()));
      return;
    }
    const target = event.target;
    const inFilter = target === filterInput;
    if (inTextField(target) && !inFilter) {
      return;
    }
    if (event.key === "ArrowDown" && grouped.flat.length > 0) {
      event.preventDefault();
      const idx = grouped.flat.findIndex((r) => r.id === selectedRepoId);
      selectedRepoId = grouped.flat[Math.min(grouped.flat.length - 1, Math.max(0, idx) + 1)].id;
    } else if (event.key === "ArrowUp" && grouped.flat.length > 0) {
      event.preventDefault();
      const idx = grouped.flat.findIndex((r) => r.id === selectedRepoId);
      selectedRepoId = grouped.flat[Math.max(0, idx < 0 ? 0 : idx - 1)].id;
    } else if (event.key === "Enter" && selectedRepoId && (inFilter || target === document.body)) {
      const repo = grouped.flat.find((r) => r.id === selectedRepoId);
      if (repo && !repo.cached?.missing) {
        event.preventDefault();
        onopen(repo.path);
      }
    } else if (event.key === "Delete" && !inFilter && selectedRepoId) {
      event.preventDefault();
      const id = selectedRepoId;
      mutate(() => libraryRemoveRepo(id));
    } else if (event.key === "Escape" && query) {
      event.preventDefault();
      query = "";
      filterInput?.blur();
    } else if (
      !inFilter &&
      !event.ctrlKey &&
      !event.altKey &&
      !event.metaKey &&
      event.key.length === 1 &&
      /\S/.test(event.key)
    ) {
      filterInput?.focus();
      tick().then(() => {
        if (filterInput && document.activeElement !== filterInput) {
          filterInput.focus();
        }
      });
    }
  }

  function handleDrop(folderId: string | null) {
    dropTarget = null;
    const dragged = dragItem;
    dragItem = null;
    if (!dragged) {
      return;
    }
    if (dragged.kind === "repo") {
      mutate(() => libraryMoveRepo(dragged.id, folderId, 0));
    } else if (dragged.id !== folderId) {
      mutate(() => libraryMoveFolder(dragged.id, folderId, 0));
    }
  }

  function hostTag(host: string | undefined): string {
    if (host === "gitHub") return "GH";
    if (host === "bitbucket") return "BB";
    if (host === "other") return "GIT";
    return "";
  }

  let libBody = $state<HTMLElement | null>(null);
  let libSideEl = $state<HTMLElement | null>(null);
  let dragStartSideW = 230;

  function measuredLibSideW(): number {
    return prefs.panes.librarySidebarWidth ?? (Math.round(libSideEl?.getBoundingClientRect().width || 0) || 230);
  }

  function clampLibSidebar(w: number): number {
    const maxW = Math.min(420, Math.max(160, (libBody?.clientWidth || 1200) - 360));
    return Math.max(160, Math.min(maxW, Math.round(w)));
  }
</script>

<svelte:window onkeydown={onKeydown} />

<main class="app lib">
  <div
    class="body"
    bind:this={libBody}
    style:--lib-side-w={prefs.panes.librarySidebarWidth != null ? `${prefs.panes.librarySidebarWidth}px` : undefined}
  >
    <nav class="side" bind:this={libSideEl} aria-label="Library navigation">
      <section class="sec">
        <button
          type="button"
          class="it"
          class:sel={selection.kind === "smart" && selection.filter === "all"}
          onclick={() => (selection = { kind: "smart", filter: "all" })}
        >
          <span class="ic">▤</span>All repositories<span class="r">{counts.all}</span>
        </button>
        <button
          type="button"
          class="it"
          class:sel={selection.kind === "smart" && selection.filter === "recent"}
          onclick={() => (selection = { kind: "smart", filter: "recent" })}
        >
          <span class="ic">◷</span>Recent<span class="r">{counts.recent || ""}</span>
        </button>
        <button
          type="button"
          class="it"
          class:sel={selection.kind === "smart" && selection.filter === "pinned"}
          onclick={() => (selection = { kind: "smart", filter: "pinned" })}
        >
          <span class="ic">★</span>Pinned<span class="r">{counts.pinned || ""}</span>
        </button>
      </section>

      <section
        class="sec"
        aria-label="Folders"
        ondragover={(e) => {
          if (dragItem) {
            e.preventDefault();
            dropTarget = "__root__";
          }
        }}
        ondrop={(e) => {
          if (e.target === e.currentTarget) handleDrop(null);
        }}
      >
        <h3>
          Folders
          <button type="button" class="bare add" onclick={() => startNewFolder(null)}>+ new</button>
        </h3>
        {#if creatingFolder && creatingFolder.parentId === null}
          <div class="inlineedit">
            <input
              type="text"
              aria-label="New folder name"
              bind:value={creatingFolder.value}
              onkeydown={(e) => {
                if (e.key === "Enter") commitNewFolder();
                else if (e.key === "Escape") creatingFolder = null;
              }}
              onblur={commitNewFolder}
            />
          </div>
        {/if}
        {#each folderNodes as node (node.folder.id)}
          {@const f = node.folder}
          {@const hasKids = library.folders.some((c) => c.parentId === f.id)}
          {#if renamingFolder?.id === f.id}
            <div class="inlineedit" style:padding-left="{10 + node.depth * 14}px">
              <input
                type="text"
                aria-label="Rename folder"
                bind:value={renamingFolder.value}
                onkeydown={(e) => {
                  if (e.key === "Enter") commitRenameFolder();
                  else if (e.key === "Escape") renamingFolder = null;
                }}
                onblur={commitRenameFolder}
              />
            </div>
          {:else}
            <button
              type="button"
              class="it"
              class:sel={selection.kind === "folder" && selection.folderId === f.id}
              class:drop={dropTarget === f.id}
              style:padding-left="{10 + node.depth * 14}px"
              draggable="true"
              ondragstart={() => (dragItem = { kind: "folder", id: f.id })}
              ondragover={(e) => {
                if (dragItem) {
                  e.preventDefault();
                  dropTarget = f.id;
                }
              }}
              ondragleave={() => dropTarget === f.id && (dropTarget = null)}
              ondrop={(e) => {
                e.preventDefault();
                e.stopPropagation();
                handleDrop(f.id);
              }}
              onclick={() => (selection = { kind: "folder", folderId: f.id })}
              ondblclick={() => {
                if (hasKids) {
                  if (collapsedFolders.has(f.id)) collapsedFolders.delete(f.id);
                  else collapsedFolders.add(f.id);
                }
              }}
              onkeydown={(e) => {
                if (e.key === "F2") {
                  e.preventDefault();
                  renamingFolder = { id: f.id, value: f.name };
                } else if (e.key === "Delete") {
                  e.preventDefault();
                  mutate(() => libraryDeleteFolder(f.id));
                }
              }}
              oncontextmenu={(e) => openFolderMenu(e, f.id, f.name)}
            >
              <span
                class="ic"
                role="presentation"
                onclick={(e) => {
                  if (hasKids) {
                    e.stopPropagation();
                    if (collapsedFolders.has(f.id)) collapsedFolders.delete(f.id);
                    else collapsedFolders.add(f.id);
                  }
                }}
              >
                {hasKids ? (collapsedFolders.has(f.id) ? "▸" : "▾") : ""}
              </span>
              <span class="nm">{f.name}</span>
              <span class="r">{node.totalCount || ""}</span>
            </button>
          {/if}
          {#if creatingFolder && creatingFolder.parentId === f.id}
            <div class="inlineedit" style:padding-left="{24 + node.depth * 14}px">
              <input
                type="text"
                aria-label="New subfolder name"
                bind:value={creatingFolder.value}
                onkeydown={(e) => {
                  if (e.key === "Enter") commitNewFolder();
                  else if (e.key === "Escape") creatingFolder = null;
                }}
                onblur={commitNewFolder}
              />
            </div>
          {/if}
        {/each}
      </section>

      <section class="sec" aria-label="Smart filters">
        <h3>Smart filters</h3>
        {#each SMART_ITEMS as item (item.id)}
          <button
            type="button"
            class="it"
            class:sel={selection.kind === "smart" && selection.filter === item.id}
            onclick={() => (selection = { kind: "smart", filter: item.id })}
          >
            <span class="ic">{#if item.id === "unity"}<UnityIcon size={10} />{:else}{item.icon}{/if}</span>{item.label}<span class="r">{counts[item.id] || ""}</span>
          </button>
        {/each}
      </section>
    </nav>

    <Splitter
      orientation="vertical"
      label="Resize library sidebar"
      onstart={() => (dragStartSideW = measuredLibSideW())}
      ondrag={(d) => setPaneSize("librarySidebarWidth", clampLibSidebar(dragStartSideW + d))}
      onstep={(d) => setPaneSize("librarySidebarWidth", clampLibSidebar(measuredLibSideW() + d))}
      onreset={() => setPaneSize("librarySidebarWidth", null)}
    />

    <section class="main" aria-label="Repositories">
      <header class="top">
        <div class="search">
          <span aria-hidden="true">⌕</span>
          <input
            bind:this={filterInput}
            type="text"
            bind:value={query}
            aria-label="Filter repositories"
            placeholder="Filter {library.repos.length} repos · name, path, branch, host · or start typing"
          />
          {#if query}
            <button type="button" class="bare ab" onclick={() => (query = "")}>✕</button>
          {:else if grouped.flat.length > 0}
            <span class="ab">Enter opens in a tab</span>
          {/if}
        </div>
        <button
          type="button"
          disabled={!!fetchingMany || grouped.flat.every((r) => r.cached?.missing)}
          title="Fetch all currently shown repositories"
          onclick={() => onfetchvisible(grouped.flat.filter((r) => !r.cached?.missing).map((r) => r.path))}
        >
          ↓ Fetch visible
        </button>
        <button type="button" aria-label="Open repository…" onclick={() => onadd(selection.kind === "folder" ? selection.folderId : null)}>
          + Add repo
        </button>
        <button type="button" onclick={onscan}>Scan a folder…</button>
        <button type="button" onclick={onclone}>Clone…</button>
        <button
          type="button"
          disabled={!library.undoAction}
          title={library.undoAction ? `Undo: ${library.undoAction} (Ctrl+Z)` : "Nothing to undo in Library"}
          aria-label={library.undoAction ? `Undo ${library.undoAction}` : "Undo library change"}
          onclick={() => mutate(libraryUndo)}
        >
          ↶
        </button>
      </header>

      <div class="lr hd" aria-hidden="true">
        <span></span>
        <span>Repository</span>
        <span>Location</span>
        <span>Branch</span>
        <span>Changes</span>
        <span>↑↓</span>
        <span>Locks</span>
        <span>Active</span>
        <span></span>
      </div>

      <div class="rows">
        {#if library.repos.length === 0}
          <div class="start empty">
            <b>gitgud</b>
            <span>Open a repository to see its history and changes.</span>
            <span class="ab">Add a repo, scan a workspace folder, or clone a repository to begin.</span>
          </div>
        {:else if grouped.groups.length === 0}
          <div class="empty ab">No repositories match this filter.</div>
        {:else}
          {#each grouped.groups as group (group.id)}
            <div class="gh">
              <b>{group.label}</b>
              <span class="ab">
                · {group.repos.length + group.hiddenCount}
                {#if group.hiddenCount > 0}
                  · {group.repos.length} recent ·
                  <button type="button" class="bare link" onclick={() => expandedGroups.add(group.id)}>show all</button>
                {/if}
              </span>
            </div>
            {#each group.repos as repo (repo.id)}
              {@const c = repo.cached}
              {@const missing = c?.missing ?? false}
              {@const isOpen = openPaths.has(pathKey(repo.path))}
              <div
                class="lr"
                class:sel={selectedRepoId === repo.id}
                class:miss={missing}
                role="button"
                tabindex="0"
                draggable="true"
                ondragstart={() => (dragItem = { kind: "repo", id: repo.id })}
                onclick={() => (selectedRepoId = repo.id)}
                ondblclick={() => !missing && onopen(repo.path)}
                onkeydown={(e) => {
                  if (e.key === "Enter" && !missing) onopen(repo.path);
                }}
                oncontextmenu={(e) => openRepoMenu(e, repo.id, repo.path, repo.pinned, missing)}
              >
                <span class="st">
                  {#if missing}
                    <span class="bad" title="Repository folder missing">⚠</span>
                  {:else if c && c.dirtyCount > 0}
                    <span class="dot" class:hollow={c.dirtyCount === c.untrackedCount} title={changesTitle(c)}></span>
                  {:else if isOpen}
                    <span class="ab" title="Open in tab">⇥</span>
                  {/if}
                </span>
                <span class="nm">
                  <button
                    type="button"
                    class="bare pin"
                    class:pinned={repo.pinned}
                    aria-label={repo.pinned ? `Unpin ${repo.name}` : `Pin ${repo.name}`}
                    onclick={(e) => {
                      e.stopPropagation();
                      mutate(() => libraryPinRepo(repo.id, !repo.pinned));
                    }}
                  >
                    {repo.pinned ? "★" : "☆"}
                  </button>
                  <span class="rname">{repo.name}</span>
                  {#if c?.isUnity}<span class="unity"><UnityIcon size={10} title="Unity project" /></span>{/if}
                </span>
                <span class="pa" title={repo.path}>
                  {#if missing}
                    <b class="bad">Not found at {repo.path}</b>
                    · <button type="button" class="bare link" onclick={(e) => { e.stopPropagation(); onlocate(repo.id); }}>Locate…</button>
                    · <button type="button" class="bare link" onclick={(e) => { e.stopPropagation(); mutate(() => libraryRemoveRepo(repo.id)); }}>Remove from gitgud</button>
                  {:else}
                    {repo.path}
                  {/if}
                </span>
                <span class="br">{c?.branch ?? "—"}</span>
                <span class="ch" title={c && c.dirtyCount > 0 ? changesTitle(c) : undefined}>
                  {#if c && c.dirtyCount > c.untrackedCount}
                    {c.dirtyCount} changed
                  {:else if c && c.untrackedCount > 0}
                    {c.untrackedCount} untracked
                  {/if}
                </span>
                <span class="ab2" title={c?.fetchedAt ? `Last fetched ${age(c.fetchedAt)} ago` : undefined}>
                  {#if c && (c.ahead || c.behind)}
                    {c.ahead ? `↑${c.ahead} ` : ""}{c.behind ? `↓${c.behind}` : ""}{#if c.behind && c.fetchedAt} · {age(c.fetchedAt)}{/if}
                  {/if}
                </span>
                <span>{c && c.locksHeld > 0 ? `🔒 ${c.locksHeld}` : ""}</span>
                <span class="la">{c?.lastActivity ? age(c.lastActivity) : ""}</span>
                <span class="host">{hostTag(c?.host)}</span>
              </div>
            {/each}
          {/each}
        {/if}
      </div>
    </section>
  </div>

  <footer class="strip">
    {#if fetchingMany}
      <span role="status">
        Fetching visible repositories… {fetchingMany.done} of {fetchingMany.total}
      </span>
    {:else}
      <span>
        {library.repos.length} {library.repos.length === 1 ? "repository" : "repositories"} · status local only, refreshed for visible rows
      </span>
    {/if}
    <span class="sp"></span>
    {#if active}
      {#await git}
        <span>Looking for git…</span>
      {:then info}
        <span class="git" data-testid="git-info" title={info.path}>git {info.version}{info.source === "bundled" ? " (bundled)" : ""} <code>{info.path}</code></span>
      {:catch err}
        <span role="alert" class="bad">{err}</span>
      {/await}
    {/if}
  </footer>
</main>

{#if menu}
  <ContextMenu x={menu.x} y={menu.y} items={menu.items} onclose={() => (menu = null)} />
{/if}

<style>
  .app.lib {
    flex: 1;
    min-height: 0;
    height: 100%;
    display: flex;
    flex-direction: column;
    background: var(--bg);
  }
  .body {
    --lib-side-w: 230px;
    flex: 1;
    min-height: 0;
    display: grid;
    grid-template-columns: var(--lib-side-w) 5px minmax(0, 1fr);
  }
  .side {
    background: var(--panel);
    overflow-y: auto;
    padding: 6px 0;
    display: grid;
    align-content: start;
    gap: 8px;
  }
  .sec {
    display: grid;
  }
  h3 {
    display: flex;
    justify-content: space-between;
    align-items: center;
    margin: 0;
    padding: 4px 10px;
    font: 600 10.5px var(--mono);
    text-transform: uppercase;
    color: var(--muted);
  }
  .add {
    font: 500 11px var(--body);
    text-transform: none;
    color: var(--l0);
    padding: 0 4px;
    border-radius: 3px;
  }
  .add:hover {
    background: var(--hover);
  }
  .it {
    width: 100%;
    display: flex;
    align-items: center;
    gap: 6px;
    min-height: var(--row-h-sm, 22px);
    padding: 2px 10px;
    border: 0;
    background: transparent;
    color: var(--fg);
    text-align: left;
    font-size: 12px;
  }
  .it:hover {
    background: var(--hover);
  }
  .it.sel {
    background: var(--sel);
    font-weight: 600;
  }
  .it.drop {
    outline: 2px dashed var(--l0);
    outline-offset: -2px;
  }
  .it .ic {
    width: 14px;
    text-align: center;
    color: var(--muted);
    flex: none;
  }
  .it .nm {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .it .r {
    margin-left: auto;
    color: var(--muted);
    font: 11px var(--mono);
  }
  .inlineedit {
    padding: 2px 8px;
  }
  .inlineedit input {
    width: 100%;
    font: inherit;
    padding: 3px 6px;
    border: 1px solid var(--l0);
    border-radius: 4px;
    background: var(--bg);
    color: var(--fg);
  }
  .main {
    display: flex;
    flex-direction: column;
    min-width: 0;
    min-height: 0;
    background: var(--panel);
  }
  .top {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 8px 10px;
    border-bottom: 1px solid var(--line);
    background: var(--panel);
  }
  .search {
    flex: 1;
    display: flex;
    align-items: center;
    gap: 6px;
    height: 26px;
    padding: 0 8px;
    border: 1px solid var(--line);
    border-radius: 5px;
    background: var(--bg);
  }
  .search input {
    flex: 1;
    border: 0;
    padding: 0;
    background: transparent;
    color: var(--fg);
    font: inherit;
    outline: none;
  }
  .search input:focus {
    background: transparent;
    border: 0;
  }
  .lr {
    display: grid;
    grid-template-columns: 20px 180px minmax(140px, 1.2fr) 150px 85px 96px 60px 80px 38px;
    align-items: center;
    gap: 8px;
    min-height: var(--row-h, 24px);
    padding: 2px 12px;
    border-bottom: 1px solid var(--line);
    font-size: 12px;
  }
  .lr.hd {
    font: 600 10.5px var(--mono);
    text-transform: uppercase;
    color: var(--muted);
    background: var(--bg);
    padding-top: 4px;
    padding-bottom: 4px;
  }
  .lr:not(.hd):hover {
    background: var(--hover);
  }
  .lr.sel {
    background: var(--sel);
  }
  .lr.miss {
    opacity: 0.78;
  }
  .rows {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
    display: flex;
    flex-direction: column;
  }
  .gh {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 5px 12px;
    background: var(--bg);
    border-bottom: 1px solid var(--line);
    font-size: 11.5px;
  }
  .st {
    display: inline-flex;
    justify-content: center;
  }
  .dot {
    width: 7px;
    height: 7px;
    border-radius: 50%;
    background: var(--l0);
  }
  .dot.hollow {
    background: none;
    box-shadow: inset 0 0 0 1.5px var(--l0);
  }
  .nm {
    display: flex;
    align-items: center;
    gap: 5px;
    min-width: 0;
  }
  .rname {
    font-weight: 600;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .pin {
    color: var(--muted);
    opacity: 0.45;
  }
  .pin.pinned,
  .lr:hover .pin {
    opacity: 1;
  }
  .pin.pinned {
    color: var(--l4);
  }
  .unity {
    display: inline-flex;
    color: var(--muted);
  }
  .pa,
  .br {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .pa {
    color: var(--muted);
  }
  .ch,
  .ab2,
  .la,
  .host {
    font: 11.5px var(--mono);
    color: var(--muted);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .host {
    text-align: right;
  }
  .link {
    color: var(--l0);
  }
  .link:hover {
    text-decoration: underline;
  }
  .start {
    flex: 1;
  }
  .strip {
    display: flex;
    align-items: center;
    gap: 14px;
    height: 24px;
    padding: 0 10px;
    flex: none;
    background: var(--panel);
    border-top: 1px solid var(--line);
    color: var(--muted);
    font-size: 11.5px;
    white-space: nowrap;
  }
  .sp {
    flex: 1;
  }
  .git code {
    font-size: 10.5px;
  }
  .bad {
    color: var(--bad-ink);
  }
</style>
