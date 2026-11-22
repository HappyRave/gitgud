<script lang="ts">
  import { onMount } from "svelte";
  import {
    closeRepo,
    commit,
    gitInfo,
    launchPath,
    onRepoChanged,
    openRepo,
    pickFolder,
    cancelFetch,
    checkout,
    discard,
    discardHunk,
    fetchRemotes,
    onFetchProgress,
    redo,
    refreshRepo,
    restoreFiles,
    restoreTo,
    repoStatus,
    undo,
    hunkHeader,
    stage,
    stageHunk,
    unstage,
    unstageHunk,
    type Hunk,
    type OpEntry,
    type Opened,
    type Progress,
    type Side,
    type Status,
  } from "$lib/api";
  import CommitBox from "$lib/components/CommitBox.svelte";
  import CommitDetail from "$lib/components/CommitDetail.svelte";
  import Graph from "$lib/components/Graph.svelte";
  import StatusList from "$lib/components/StatusList.svelte";
  import WorkingFileDiff from "$lib/components/WorkingFileDiff.svelte";
  import OpLogPanel from "$lib/components/OpLogPanel.svelte";
  import Sidebar from "$lib/components/Sidebar.svelte";
  import { isFetchKey, opLogKey, paneKey } from "$lib/keys";

  const git = gitInfo();

  let opened = $state<Opened | null>(null);
  let status = $state<Status | null>(null);
  let error = $state<string | null>(null);
  let loading = $state(false);
  /** The commit shown in the side pane; `null` shows the working copy. */
  let selectedCommit = $state<string | null>(null);
  let selectedRow = $state<number | null>(null);

  function closeDetail() {
    selectedCommit = null;
    selectedRow = null;
  }

  /** The working-copy file whose diff is shown. */
  let selectedFile = $state<{ path: string; side: Side } | null>(null);
  /** Bumped on every status read, so the shown diff reloads with it. */
  let revision = $state(0);

  const stagedCount = $derived(status?.entries.filter((e) => e.index).length ?? 0);

  /** Whether the selected file's hunks can be staged one at a time. */
  const perHunk = $derived.by(() => {
    const entry = status?.entries.find((e) => e.path === selectedFile?.path);
    if (!entry || entry.kind !== "tracked") {
      return false;
    }
    return selectedFile?.side === "staged" ? entry.index === "modified" : entry.worktree === "modified";
  });

  /** Shows a (re)opened repo. A new or rebuilt graph drops the selection with its rows. */
  function show(next: Opened, nextStatus: Status) {
    if (next.id !== opened?.id || next.graphVersion !== opened.graphVersion) {
      closeDetail();
    }
    if (next.id !== opened?.id || (selectedFile && !hasSide(nextStatus, selectedFile.path, selectedFile.side))) {
      selectedFile = null;
    }
    opened = next;
    status = nextStatus;
    revision++;
  }

  function hasSide(s: Status, path: string, side: Side) {
    const entry = s.entries.find((e) => e.path === path);
    return side === "staged" ? !!entry?.index : !!entry && (!!entry.worktree || entry.kind === "untracked");
  }

  function onHunk(action: "stage" | "unstage" | "discard", hunk: Hunk) {
    const file = selectedFile;
    if (!file) {
      return;
    }
    const header = hunkHeader(hunk);
    const run = { stage: stageHunk, unstage: unstageHunk, discard: discardHunk }[action];
    write((id) => run(id, file.path, header));
  }

  /** Writes, undos and redos run one after another, in the order asked for. */
  let queue: Promise<unknown> = Promise.resolve();
  let pending = $state(0);
  const busy = $derived(pending > 0);

  function enqueue<T>(job: () => Promise<T>): Promise<T> {
    pending++;
    const run = queue.then(job).finally(() => pending--);
    queue = run.catch(() => {});
    return run;
  }

  /** Runs a write, then shows the refreshed repo. Resolves to whether it worked. */
  function write(run: (id: number) => Promise<Opened>): Promise<boolean> {
    return enqueue(async () => {
      if (!opened) {
        return false;
      }
      error = null;
      try {
        const next = await run(opened.id);
        show(next, await repoStatus(next.id));
        notice = null;
        return true;
      } catch (err) {
        error = String(err);
        return false;
      }
    });
  }

  /** Names the last undo or redo, e.g. "Undid: Stage a.txt". */
  let notice = $state<string | null>(null);

  function revert(kind: "undo" | "redo") {
    return enqueue(async () => {
      if (!opened) {
        return;
      }
      error = null;
      try {
        const result = kind === "undo" ? await undo(opened.id) : await redo(opened.id);
        show(result.opened, await repoStatus(result.opened.id));
        notice = `${kind === "undo" ? "Undid" : "Redid"}: ${result.action}`;
      } catch (err) {
        error = String(err);
      }
    });
  }

  /** The running fetch: `null` when idle, its latest progress line once git prints one. */
  let fetching = $state<{ progress: Progress | null } | null>(null);

  async function startFetch() {
    if (!opened || fetching) {
      return;
    }
    const id = opened.id;
    fetching = { progress: null };
    error = null;
    try {
      const next = await fetchRemotes(id);
      if (opened?.id === id) {
        show(next, await repoStatus(id));
        notice = "Fetched";
      }
    } catch (err) {
      if (String(err) === "cancelled") {
        notice = "Fetch cancelled";
      } else {
        error = String(err);
      }
    } finally {
      fetching = null;
    }
  }

  /** Which panel the side pane shows when no commit is selected. */
  let inspector = $state<"working" | "oplog">("working");

  function restoreToEntry(entry: OpEntry) {
    return enqueue(async () => {
      if (!opened) {
        return;
      }
      error = null;
      try {
        const result = await restoreTo(opened.id, entry.id);
        show(result.opened, await repoStatus(result.opened.id));
        notice = `Restored to before: ${result.action}`;
      } catch (err) {
        error = String(err);
      }
    });
  }

  function onKeydown(event: KeyboardEvent) {
    const pane = paneKey(event);
    if (pane && opened) {
      event.preventDefault();
      inspector = pane;
      closeDetail();
      return;
    }
    if (isFetchKey(event) && opened) {
      event.preventDefault();
      startFetch();
      return;
    }
    const kind = opLogKey(event);
    if (kind && opened) {
      event.preventDefault();
      revert(kind);
    }
  }

  async function onCommit(message: string) {
    if (!(await write((id) => commit(id, message)))) {
      throw new Error("commit failed");
    }
  }

  let refreshing = false;
  let refreshAgain = false;

  /** Outside changes: one refresh at a time, and one more if changes arrived meanwhile. */
  async function onOutsideChange(id: number) {
    if (opened?.id !== id || busy) {
      return;
    }
    if (refreshing) {
      refreshAgain = true;
      return;
    }
    refreshing = true;
    try {
      do {
        refreshAgain = false;
        const next = await refreshRepo(id);
        const nextStatus = await repoStatus(id);
        if (opened?.id === id) {
          show(next, nextStatus);
        }
      } while (refreshAgain);
    } catch (err) {
      error = String(err);
    } finally {
      refreshing = false;
    }
  }

  onMount(() => {
    const stops: (() => void)[] = [];
    let destroyed = false;
    const subscriptions = [
      onRepoChanged(onOutsideChange),
      onFetchProgress((id, progress) => {
        if (fetching && opened?.id === id) {
          fetching = { progress };
        }
      }),
    ];
    for (const subscription of subscriptions) {
      subscription.then((unlisten) => (destroyed ? unlisten() : stops.push(unlisten)));
    }
    return () => {
      destroyed = true;
      stops.forEach((stop) => stop());
    };
  });

  async function load(path: string) {
    loading = true;
    error = null;
    try {
      const next = await openRepo(path);
      const nextStatus = await repoStatus(next.id);
      if (opened) {
        await closeRepo(opened.id);
      }
      show(next, nextStatus);
    } catch (err) {
      error = String(err);
    } finally {
      loading = false;
    }
  }

  async function chooseRepo() {
    const path = await pickFolder();
    if (path) {
      await load(path);
    }
  }

  launchPath().then(
    (path) => {
      if (path) {
        load(path);
      }
    },
    (err) => (error = String(err)),
  );
</script>

<svelte:window onkeydown={onKeydown} />

<main>
  <header>
    <h1>gitgud</h1>
    <button onclick={chooseRepo}>Open repository…</button>
    {#if opened}
      <button disabled={busy} title="Ctrl+Z" onclick={() => revert("undo")}>Undo</button>
      <button disabled={busy} title="Ctrl+Y" onclick={() => revert("redo")}>Redo</button>
      <button disabled={!!fetching} title="Ctrl+Shift+F" onclick={startFetch}>Fetch</button>
    {/if}
    {#await git}
      <span class="git">Looking for git…</span>
    {:then info}
      <span class="git" data-testid="git-info">git {info.version} <code>{info.path}</code></span>
    {:catch err}
      <span role="alert">{err}</span>
    {/await}
  </header>

  {#if fetching && opened}
    {@const id = opened.id}
    <p class="task" aria-label="Fetch progress">
      Fetching…
      {#if fetching.progress}
        {fetching.progress.phase} {fetching.progress.percent}%
      {/if}
      <button onclick={() => cancelFetch(id)}>Cancel</button>
    </p>
  {/if}

  {#if error}
    <p role="alert">{error}</p>
  {:else if notice}
    <p role="status">{notice}</p>
  {/if}

  {#if opened && status}
    <div class="repo">
      <Sidebar
        repoId={opened.id}
        {revision}
        {busy}
        oplogOpen={inspector === "oplog" && !selectedCommit}
        oncheckout={(branch) => write((id) => checkout(id, branch))}
        onoplog={() => {
          inspector = inspector === "oplog" ? "working" : "oplog";
          closeDetail();
        }}
      />
      {#key `${opened.id}:${opened.graphVersion}`}
        <section class="center" aria-label="History">
          <Graph
            repoId={opened.id}
            rowCount={opened.rows}
            bind:selected={selectedRow}
            onselect={(row) => (selectedCommit = row && !row.wip ? row.id : null)}
          />
        </section>
      {/key}
      <aside>
        {#if selectedCommit}
          <CommitDetail repoId={opened.id} commit={selectedCommit} onclose={closeDetail} />
        {:else if inspector === "oplog"}
          <OpLogPanel
            repoId={opened.id}
            {revision}
            {busy}
            onundo={() => revert("undo")}
            onrestoreto={restoreToEntry}
            onrestorefiles={(entry) => write((id) => restoreFiles(id, entry.id))}
          />
        {:else}
          <StatusList
            {status}
            {busy}
            selected={selectedFile}
            onstage={(paths) => write((id) => stage(id, paths))}
            onunstage={(paths) => write((id) => unstage(id, paths))}
            onselectfile={(path, side) => (selectedFile = { path, side })}
            ondiscard={(paths) => write((id) => discard(id, paths))}
          />
          <CommitBox staged={stagedCount} {busy} oncommit={onCommit} />
          {#if selectedFile}
            <WorkingFileDiff
              repoId={opened.id}
              path={selectedFile.path}
              side={selectedFile.side}
              {revision}
              {perHunk}
              {busy}
              onhunk={onHunk}
            />
          {/if}
        {/if}
      </aside>
    </div>
  {:else if loading}
    <p>Opening…</p>
  {:else if !error}
    <p class="hint">Open a repository to see its history and changes.</p>
  {/if}
</main>

<style>
  :global(html, body) {
    margin: 0;
    height: 100%;
  }
  main {
    font-family: system-ui, sans-serif;
    height: 100vh;
    display: flex;
    flex-direction: column;
    box-sizing: border-box;
    padding: 0.75rem 1rem 0;
  }
  header {
    display: flex;
    align-items: baseline;
    gap: 1rem;
    padding-bottom: 0.5rem;
  }
  h1 {
    margin: 0;
    font-size: 1.25rem;
  }
  .git {
    color: #666;
    font-size: 0.8em;
  }
  .hint {
    color: #888;
  }
  .repo {
    flex: 1;
    min-height: 0;
    display: grid;
    grid-template-columns: 13rem minmax(0, 1fr) 26rem;
    gap: 1rem;
  }
  .center {
    min-height: 0;
  }
  aside {
    overflow-y: auto;
    border-left: 1px solid #eee;
    padding-left: 1rem;
  }
</style>
