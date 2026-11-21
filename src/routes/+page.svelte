<script lang="ts">
  import { gitInfo, launchPath, pickFolder, repoStatus, type Status } from "$lib/api";
  import StatusList from "$lib/components/StatusList.svelte";

  const git = gitInfo();

  let status = $state<Status | null>(null);
  let error = $state<string | null>(null);
  let loading = $state(false);

  async function load(path: string) {
    loading = true;
    error = null;
    try {
      status = await repoStatus(path);
    } catch (err) {
      status = null;
      error = String(err);
    } finally {
      loading = false;
    }
  }

  async function openRepo() {
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
    <button onclick={openRepo}>Open repository…</button>
  </header>

  {#await git}
    <p>Looking for git…</p>
  {:then info}
    <p class="git" data-testid="git-info">git {info.version} <code>{info.path}</code></p>
  {:catch err}
    <p role="alert">{err}</p>
  {/await}

  {#if error}
    <p role="alert">{error}</p>
  {:else if loading && !status}
    <p>Reading status…</p>
  {:else if status}
    <StatusList {status} />
  {:else}
    <p class="hint">Open a repository to see its changes.</p>
  {/if}
</main>

<style>
  main {
    font-family: system-ui, sans-serif;
    padding: 1.5rem 2rem;
  }
  header {
    display: flex;
    align-items: center;
    gap: 1rem;
  }
  h1 {
    margin: 0;
  }
  .git {
    color: #666;
    font-size: 0.85em;
  }
  .hint {
    color: #888;
  }
</style>
