<script lang="ts">
  import { closeRepo, gitInfo, launchPath, openRepo, pickFolder, repoStatus, type Opened, type Status } from "$lib/api";
  import Graph from "$lib/components/Graph.svelte";
  import StatusList from "$lib/components/StatusList.svelte";

  const git = gitInfo();

  let opened = $state<Opened | null>(null);
  let status = $state<Status | null>(null);
  let error = $state<string | null>(null);
  let loading = $state(false);

  async function load(path: string) {
    loading = true;
    error = null;
    try {
      const next = await openRepo(path);
      const nextStatus = await repoStatus(next.id);
      if (opened) {
        await closeRepo(opened.id);
      }
      opened = next;
      status = nextStatus;
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

<main>
  <header>
    <h1>gitgud</h1>
    <button onclick={chooseRepo}>Open repository…</button>
    {#await git}
      <span class="git">Looking for git…</span>
    {:then info}
      <span class="git" data-testid="git-info">git {info.version} <code>{info.path}</code></span>
    {:catch err}
      <span role="alert">{err}</span>
    {/await}
  </header>

  {#if error}
    <p role="alert">{error}</p>
  {/if}

  {#if opened && status}
    <div class="repo">
      {#key opened.id}
        <section class="center" aria-label="History">
          <Graph repoId={opened.id} rowCount={opened.rows} />
        </section>
      {/key}
      <aside>
        <StatusList {status} />
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
    grid-template-columns: minmax(0, 1fr) 20rem;
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
