<script lang="ts">
  import { onDestroy, onMount } from "svelte";
  import {
    gitInfo,
    launchPath,
    libraryAddRepos,
    libraryApplyScan,
    libraryLoad,
    libraryRescanFolder,
    libraryLocateRepo,
    librarySaveTabs,
    librarySummaries,
    fetchMany,
    onFetchManyProgress,
    pickFolder,
    scanFolder,
    setWindowTitle,
    type AddRepoEntry,
    type FetchManyProgress,
    type LibrarySnapshot,
    type ScanResult,
  } from "$lib/api";
  import CloneDialog from "$lib/components/CloneDialog.svelte";
  import CommandPalette from "$lib/components/CommandPalette.svelte";
  import ErrorBanner from "$lib/components/ErrorBanner.svelte";
  import LibraryView from "$lib/components/LibraryView.svelte";
  import QuickSwitch from "$lib/components/QuickSwitch.svelte";
  import RepoView from "$lib/components/RepoView.svelte";
  import ScanFolderDialog from "$lib/components/ScanFolderDialog.svelte";
  import SettingsModal from "$lib/components/SettingsModal.svelte";
  import ShortcutSheet from "$lib/components/ShortcutSheet.svelte";
  import TabBar, { type RepoTabInfo } from "$lib/components/TabBar.svelte";
  import UpdateDialog from "$lib/components/UpdateDialog.svelte";
  import { blockBrowserMenu } from "$lib/browserMenu";
  import { checkForUpdates, dismissOffer, skipOffer, updates } from "$lib/updates.svelte";
  import { checkLatestGit } from "$lib/gitLatest.svelte";
  import {
    isCloseTabKey,
    isCommandPaletteKey,
    isQuickSwitchKey,
    isSettingsKey,
    isShortcutSheetKey,
    tabCycleKey,
  } from "$lib/keys";
  import { folderPathLabel, pathKey } from "$lib/library/filter";
  import { applyDensity, applyTheme, prefs } from "$lib/prefs.svelte";

  const git = gitInfo();

  const EMPTY_LIBRARY: LibrarySnapshot = {
    version: 1,
    folders: [],
    repos: [],
    tabs: { openPaths: [], activePath: null },
    undoAction: null,
    redoAction: null,
  };

  let library = $state<LibrarySnapshot>(EMPTY_LIBRARY);
  let openTabs = $state<RepoTabInfo[]>([]);
  let activePath = $state<string | null>(null);
  let prevTabOnOpen = $state<string | null>(null);
  let booted = $state(false);
  let globalError = $state<string | null>(null);

  let quickSwitchOpen = $state(false);
  let paletteOpen = $state(false);
  let shortcutsOpen = $state(false);
  let settingsOpen = $state(false);
  let scanResult = $state<ScanResult | null>(null);
  /** The Library folder the open scan dialog rescans, if it is a rescan. */
  let rescanOf = $state<string | null>(null);
  let cloneOpen = $state(false);
  let cloneInitialFolder = $state("");
  let fetchingMany = $state<FetchManyProgress | null>(null);
  let repoViews = $state<Record<string, ReturnType<typeof RepoView> | undefined>>({});

  const openPathSet = $derived(new Set(openTabs.map((t) => pathKey(t.path))));
  const appModalOpen = $derived(
    quickSwitchOpen || paletteOpen || shortcutsOpen || settingsOpen || !!scanResult || cloneOpen,
  );
  const activeCtx = $derived(
    activePath !== null ? (repoViews[pathKey(activePath)]?.getPaletteContext() ?? null) : null,
  );

  $effect(() => {
    applyTheme(prefs.theme);
    applyDensity(prefs.density);
  });

  function repoName(path: string) {
    const parts = path.split(/[\\/]/).filter(Boolean);
    return parts[parts.length - 1] ?? path;
  }

  function applySummaries(summaries: LibrarySnapshot["repos"][number]["cached"][]) {
    const byKey = new Map(summaries.filter((s): s is NonNullable<typeof s> => !!s).map((s) => [pathKey(s.path), s]));
    if (byKey.size === 0) {
      return;
    }
    library = {
      ...library,
      repos: library.repos.map((r) => {
        const s = byKey.get(pathKey(r.path));
        return s ? { ...r, cached: s } : r;
      }),
    };
    openTabs = openTabs.map((t) => {
      const s = byKey.get(pathKey(t.path));
      return s ? { ...t, dirtyCount: s.dirtyCount, locksHeld: s.locksHeld } : t;
    });
  }

  function makeTabInfo(path: string): RepoTabInfo {
    const k = pathKey(path);
    const libRepo = library.repos.find((r) => pathKey(r.path) === k);
    return {
      path,
      name: libRepo?.name ?? repoName(path),
      dirtyCount: libRepo?.cached?.dirtyCount ?? 0,
      locksHeld: libRepo?.cached?.locksHeld ?? 0,
    };
  }

  function persistTabs() {
    if (!booted) {
      return;
    }
    librarySaveTabs({
      openPaths: openTabs.map((t) => t.path),
      activePath,
    }).catch((err) => console.error("save tabs:", err));
  }

  function openRepoTab(path: string) {
    quickSwitchOpen = false;
    globalError = null;
    const k = pathKey(path);
    const existing = openTabs.find((t) => pathKey(t.path) === k);
    if (existing) {
      activePath = existing.path;
      persistTabs();
      return;
    }
    prevTabOnOpen = activePath;
    openTabs = [...openTabs, makeTabInfo(path)];
    activePath = path;
    persistTabs();
  }

  function selectTab(path: string | null) {
    activePath = path;
    persistTabs();
  }

  function closeTab(path: string) {
    const k = pathKey(path);
    const idx = openTabs.findIndex((t) => pathKey(t.path) === k);
    if (idx < 0) {
      return;
    }
    const next = openTabs.filter((_, i) => i !== idx);
    openTabs = next;
    if (activePath !== null && pathKey(activePath) === k) {
      const fallback = next[Math.min(idx, next.length - 1)]?.path ?? null;
      activePath = fallback;
    }
    persistTabs();
  }

  function onTabOpenFailed(path: string, message: string) {
    const k = pathKey(path);
    openTabs = openTabs.filter((t) => pathKey(t.path) !== k);
    if (activePath !== null && pathKey(activePath) === k) {
      activePath =
        prevTabOnOpen && openTabs.some((t) => pathKey(t.path) === pathKey(prevTabOnOpen!)) ? prevTabOnOpen : null;
    }
    globalError = message;
    persistTabs();
  }

  function updateTabDirty(path: string, dirtyCount: number) {
    const k = pathKey(path);
    openTabs = openTabs.map((t) => (pathKey(t.path) === k ? { ...t, dirtyCount } : t));
  }

  async function chooseAndOpenRepo(folderId?: string | null) {
    const picked = await pickFolder();
    if (!picked) {
      return;
    }
    openRepoTab(picked);
    if (!library.repos.some((r) => pathKey(r.path) === pathKey(picked))) {
      const folderPath = folderId ? folderPathLabel(library.folders, folderId) : null;
      libraryAddRepos([{ path: picked, folderPath }])
        .then((snap) => (library = snap))
        .catch(() => {});
    }
  }

  async function runScanFolder() {
    quickSwitchOpen = false;
    const root = await pickFolder("Scan folder for Git repositories");
    if (!root) {
      return;
    }
    try {
      scanResult = await scanFolder(root);
    } catch (err) {
      globalError = String(err);
    }
  }

  async function rescanFolder(folderId: string) {
    try {
      scanResult = await libraryRescanFolder(folderId);
      rescanOf = folderPathLabel(library.folders, folderId);
    } catch (err) {
      globalError = String(err);
    }
  }

  async function addScannedRepos(entries: AddRepoEntry[], removeIds: string[]) {
    scanResult = null;
    rescanOf = null;
    try {
      library = removeIds.length > 0 ? await libraryApplyScan(entries, removeIds) : await libraryAddRepos(entries);
    } catch (err) {
      globalError = String(err);
    }
  }

  async function locateMissingRepo(id: string) {
    const picked = await pickFolder("Locate moved repository");
    if (!picked) {
      return;
    }
    try {
      library = await libraryLocateRepo(id, picked);
    } catch (err) {
      globalError = String(err);
    }
  }

  async function refreshSummaries(paths: string[]) {
    if (paths.length === 0) {
      return;
    }
    try {
      applySummaries(await librarySummaries(paths));
    } catch (err) {
      globalError = String(err);
    }
  }

  async function runFetchVisible(paths: string[]) {
    if (paths.length === 0 || fetchingMany) {
      return;
    }
    globalError = null;
    fetchingMany = { done: 0, total: paths.length, path: "", error: null };
    try {
      applySummaries(await fetchMany(paths));
    } catch (err) {
      globalError = String(err);
    } finally {
      fetchingMany = null;
    }
  }

  function onGlobalKeydown(event: KeyboardEvent) {
    if (event.key === "F5" || event.key === "F7") {
      event.preventDefault();
      return;
    }
    if (isCommandPaletteKey(event)) {
      event.preventDefault();
      quickSwitchOpen = false;
      shortcutsOpen = false;
      settingsOpen = false;
      paletteOpen = !paletteOpen;
      return;
    }
    if (isSettingsKey(event)) {
      event.preventDefault();
      quickSwitchOpen = false;
      paletteOpen = false;
      shortcutsOpen = false;
      settingsOpen = !settingsOpen;
      return;
    }
    if (isQuickSwitchKey(event)) {
      event.preventDefault();
      paletteOpen = false;
      shortcutsOpen = false;
      settingsOpen = false;
      quickSwitchOpen = !quickSwitchOpen;
      return;
    }
    if (isShortcutSheetKey(event) && !appModalOpen) {
      event.preventDefault();
      shortcutsOpen = true;
      return;
    }
    if (appModalOpen) {
      return;
    }
    if (isCloseTabKey(event) && activePath !== null) {
      event.preventDefault();
      closeTab(activePath);
      return;
    }
    const dir = tabCycleKey(event);
    if (dir !== null && openTabs.length > 0) {
      event.preventDefault();
      const order: (string | null)[] = [null, ...openTabs.map((t) => t.path)];
      const curIdx = activePath === null ? 0 : order.findIndex((p) => p !== null && pathKey(p) === pathKey(activePath!));
      const nextIdx = (Math.max(0, curIdx) + dir + order.length) % order.length;
      selectTab(order[nextIdx]);
    }
  }

  $effect(() => {
    if (activePath === null && booted) {
      setWindowTitle("gitgud · Library").catch(() => {});
    }
  });

  let stopFetchMany: (() => void) | undefined;
  // Git for Windows' version check waits until the window and the first repo have shown.
  const GIT_CHECK_DELAY_MS = 3000;
  let gitCheckTimer: ReturnType<typeof setTimeout> | undefined;

  onMount(async () => {
    stopFetchMany = await onFetchManyProgress((p) => (fetchingMany = p));
    void checkForUpdates(false);
    const [launchRes, libRes] = await Promise.allSettled([launchPath(), libraryLoad()]);
    if (libRes.status === "fulfilled") {
      library = libRes.value;
    }
    booted = true;
    const launch = launchRes.status === "fulfilled" ? launchRes.value : null;
    if (launch) {
      openRepoTab(launch);
    } else if (library.tabs.openPaths.length > 0) {
      openTabs = library.tabs.openPaths.map(makeTabInfo);
      const savedActive = library.tabs.activePath;
      activePath =
        savedActive && openTabs.some((t) => pathKey(t.path) === pathKey(savedActive)) ? savedActive : openTabs[0].path;
    }
    gitCheckTimer = setTimeout(() => void checkLatestGit(false), GIT_CHECK_DELAY_MS);
  });

  onDestroy(() => {
    stopFetchMany?.();
    clearTimeout(gitCheckTimer);
  });
</script>

<svelte:window onkeydown={onGlobalKeydown} oncontextmenu={blockBrowserMenu} />

<div class="shell">
  <TabBar
    tabs={openTabs}
    {activePath}
    onselect={selectTab}
    onclose={closeTab}
    onadd={() => chooseAndOpenRepo()}
    onquickswitch={() => (quickSwitchOpen = true)}
    onpalette={() => (paletteOpen = true)}
    onshortcuts={() => (shortcutsOpen = true)}
    onsettings={() => (settingsOpen = true)}
  />

  {#if globalError}
    <ErrorBanner
      message={globalError}
      onfix={(fix) => fix === "settings" && (settingsOpen = true)}
      ondismiss={() => (globalError = null)}
    />
  {/if}

  <div class="views">
    <div class="view" class:off={activePath !== null} hidden={activePath !== null}>
      <LibraryView
        active={activePath === null}
        suspended={appModalOpen}
        {library}
        openPaths={openPathSet}
        {git}
        {fetchingMany}
        onopen={openRepoTab}
        onadd={(folderId) => chooseAndOpenRepo(folderId)}
        onscan={runScanFolder}
        onrescan={rescanFolder}
        onclone={() => {
          cloneInitialFolder = "";
          cloneOpen = true;
        }}
        onlocate={locateMissingRepo}
        onfetchvisible={runFetchVisible}
        onlibrarychange={(snap) => (library = snap)}
        onrefreshsummaries={refreshSummaries}
        onerror={(msg) => (globalError = msg)}
      />
    </div>

    {#each openTabs as tab (pathKey(tab.path))}
      {@const isTabActive = activePath !== null && pathKey(activePath) === pathKey(tab.path)}
      <div class="view" class:off={!isTabActive} hidden={!isTabActive}>
        <RepoView
          bind:this={repoViews[pathKey(tab.path)]}
          path={tab.path}
          active={isTabActive}
          suspended={appModalOpen}
          {git}
          onopenother={() => chooseAndOpenRepo()}
          onstatuschange={(dirty) => updateTabDirty(tab.path, dirty)}
          onopenfail={(msg) => onTabOpenFailed(tab.path, msg)}
          onopensettings={() => (settingsOpen = true)}
        />
      </div>
    {/each}
  </div>
</div>

{#if quickSwitchOpen}
  <QuickSwitch
    repos={library.repos}
    folders={library.folders}
    openPaths={openPathSet}
    onopen={openRepoTab}
    onaddrepo={() => {
      quickSwitchOpen = false;
      chooseAndOpenRepo();
    }}
    onscanfolder={runScanFolder}
    onclone={() => {
      quickSwitchOpen = false;
      cloneOpen = true;
    }}
    onclose={() => (quickSwitchOpen = false)}
  />
{/if}

{#if scanResult}
  <ScanFolderDialog
    scan={scanResult}
    {rescanOf}
    onadd={addScannedRepos}
    oncancel={() => {
      scanResult = null;
      rescanOf = null;
    }}
  />
{/if}

{#if cloneOpen}
  <CloneDialog
    initialFolderPath={cloneInitialFolder}
    oncloned={async (clonedPath) => {
      cloneOpen = false;
      library = await libraryLoad().catch(() => library);
      openRepoTab(clonedPath);
    }}
    oncancel={() => (cloneOpen = false)}
  />
{/if}

{#if paletteOpen}
  <CommandPalette
    repo={activeCtx?.actions ?? null}
    onopenrepo={() => chooseAndOpenRepo()}
    onscanfolder={runScanFolder}
    onclone={() => {
      cloneInitialFolder = "";
      cloneOpen = true;
    }}
    onquickswitch={() => (quickSwitchOpen = true)}
    onshortcuts={() => (shortcutsOpen = true)}
    onsettings={() => (settingsOpen = true)}
    onclose={() => (paletteOpen = false)}
  />
{/if}

{#if shortcutsOpen}
  <ShortcutSheet onclose={() => (shortcutsOpen = false)} />
{/if}

{#if settingsOpen}
  <SettingsModal
    activeRepoId={activeCtx?.repoId ?? null}
    activeRepoName={activeCtx?.repoName ?? null}
    onclose={() => (settingsOpen = false)}
  />
{/if}

{#if updates.offer}
  <UpdateDialog offer={updates.offer} onskip={skipOffer} onlater={dismissOffer} />
{/if}

<style>
  .shell {
    height: 100vh;
    display: flex;
    flex-direction: column;
    background: var(--bg);
  }
  .views {
    flex: 1;
    min-height: 0;
    display: flex;
    flex-direction: column;
  }
  .view {
    flex: 1;
    min-height: 0;
    display: flex;
    flex-direction: column;
  }
  .view.off {
    display: none;
  }
</style>
