<script lang="ts">
  import { onDestroy, onMount } from "svelte";
  import { cancelClone, cloneRepo, onCloneProgress, pickFolder, type CloneProgress } from "$lib/api";
  import Dialog from "./Dialog.svelte";

  let {
    initialFolderPath = "",
    oncloned,
    oncancel,
  }: {
    initialFolderPath?: string;
    oncloned: (path: string) => void;
    oncancel: () => void;
  } = $props();

  let url = $state("");
  let dest = $state("");
  let destEdited = $state(false);
  let folderPath = $state("");
  let longPaths = $state(true);
  /** R10-B: pointers first, then what the build's scenes use. Repos without Unity scenes still get every file. */
  let lfsPartial = $state(true);
  let cloning = $state(false);
  let progress = $state<CloneProgress | null>(null);
  let error = $state<string | null>(null);
  let urlInput = $state<HTMLInputElement | null>(null);
  let stopProgress: (() => void) | undefined;

  function inferRepoName(raw: string): string {
    const clean = raw.trim().replace(/[\\/]+$/, "").replace(/\.git$/i, "");
    const tail = clean.split(/[/:\\]/).pop() ?? "";
    return tail.trim();
  }

  onMount(async () => {
    folderPath = initialFolderPath;
    stopProgress = await onCloneProgress((p) => (progress = p));
    requestAnimationFrame(() => urlInput?.focus());
  });

  onDestroy(() => {
    stopProgress?.();
  });

  async function pickDestParent() {
    const parent = await pickFolder("Choose parent folder");
    if (!parent) {
      return;
    }
    const name = inferRepoName(url);
    const sep = parent.includes("\\") ? "\\" : "/";
    const base = parent.replace(/[\\/]+$/, "");
    dest = name ? `${base}${sep}${name}` : base;
    destEdited = true;
  }

  function updateDestFromUrl(nextUrl: string) {
    url = nextUrl;
    if (!destEdited && dest) {
      const name = inferRepoName(nextUrl);
      const parent = dest.replace(/[\\/][^\\/]*$/, "");
      if (name && parent) {
        const sep = dest.includes("\\") ? "\\" : "/";
        dest = `${parent}${sep}${name}`;
      }
    }
  }

  const valid = $derived(url.trim().length > 0 && dest.trim().length > 0);

  async function submit(event: SubmitEvent) {
    event.preventDefault();
    if (!valid || cloning) {
      return;
    }
    cloning = true;
    error = null;
    progress = null;
    try {
      const cloned = await cloneRepo(url.trim(), dest.trim(), folderPath.trim() || null, longPaths, lfsPartial);
      oncloned(cloned);
    } catch (err) {
      const msg = String(err);
      if (msg !== "cancelled") {
        error = msg;
      }
    } finally {
      cloning = false;
    }
  }

  async function abortClone() {
    if (cloning) {
      await cancelClone();
    } else {
      oncancel();
    }
  }
</script>

<Dialog title="Clone a repository" width={500} oncancel={abortClone}>
  <form onsubmit={submit}>
    {#if error}
      <p role="alert" class="flag bad">{error}</p>
    {/if}

    <label class="field">
      <span class="ab">Repository URL</span>
      <input
        bind:this={urlInput}
        type="text"
        value={url}
        disabled={cloning}
        oninput={(e) => updateDestFromUrl((e.currentTarget as HTMLInputElement).value)}
        placeholder="https://github.com/org/repo.git or git@github.com:org/repo.git"
        aria-label="Repository URL"
      />
    </label>

    <div class="field">
      <span class="ab">Destination folder</span>
      <div class="row">
        <input
          type="text"
          value={dest}
          disabled={cloning}
          oninput={(e) => {
            dest = (e.currentTarget as HTMLInputElement).value;
            destEdited = true;
          }}
          placeholder="D:\Work\repo"
          aria-label="Destination folder"
        />
        <button type="button" disabled={cloning} onclick={pickDestParent}>Browse…</button>
      </div>
    </div>

    <label class="field">
      <span class="ab">Library folder (optional)</span>
      <input type="text" bind:value={folderPath} disabled={cloning} placeholder="Studio / VR" aria-label="Library folder" />
    </label>

    <label class="chk">
      <input type="checkbox" bind:checked={longPaths} disabled={cloning} />
      <span>Enable Windows long paths (<code>core.longpaths</code>)</span>
    </label>

    <label class="chk">
      <input type="checkbox" bind:checked={lfsPartial} disabled={cloning} />
      <span>LFS: download only what the build's scenes use (change it later from Ctrl+K › LFS downloads)</span>
    </label>

    {#if cloning}
      <div class="prog" role="status" aria-label="Clone progress">
        <div class="pline">
          <span>{progress?.phase ?? "Starting clone…"}</span>
          <span class="ab">
            {#if progress?.lfs}
              LFS {progress.lfs.filesDone}/{progress.lfs.filesTotal} ({Math.round(progress.lfs.bytesDone / 1048576)} / {Math.max(1, Math.round(progress.lfs.bytesTotal / 1048576))} MB) ·
            {/if}
            {progress?.percent ?? 0}%
          </span>
        </div>
        <div class="bar"><i style:width="{progress?.percent ?? 0}%"></i></div>
      </div>
    {/if}

    <div class="bt">
      <span class="ab">Runs git clone --progress (with LFS)</span>
      <span class="sp"></span>
      <button type="button" onclick={abortClone}>Cancel</button>
      <button type="submit" class="pri" disabled={!valid || cloning}>
        {cloning ? "Cloning…" : "Clone"} <span class="k">↵</span>
      </button>
    </div>
  </form>
</Dialog>

<style>
  form {
    display: grid;
    gap: 10px;
  }
  .field {
    display: grid;
    gap: 4px;
  }
  .row {
    display: flex;
    gap: 6px;
  }
  .row input {
    flex: 1;
  }
  .chk {
    display: flex;
    gap: 6px;
    align-items: center;
  }
  .prog {
    display: grid;
    gap: 5px;
    padding: 8px 10px;
    border: 1px solid var(--line);
    border-radius: 6px;
    background: var(--bg);
  }
  .pline {
    display: flex;
    justify-content: space-between;
    font-size: 12px;
  }
  .bar {
    height: 6px;
    border-radius: 3px;
    background: var(--line);
    overflow: hidden;
  }
  .bar i {
    display: block;
    height: 100%;
    background: var(--l0);
  }
  .bt {
    display: flex;
    gap: 6px;
    align-items: center;
  }
  .sp {
    flex: 1;
  }
  p {
    margin: 0;
  }
</style>
