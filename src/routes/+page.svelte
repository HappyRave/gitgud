<script lang="ts">
  import { gitInfo } from "$lib/api";

  const info = gitInfo();
</script>

<main>
  <h1>gitgud</h1>
  {#await info}
    <p>Looking for git…</p>
  {:then git}
    <p data-testid="git-info">git {git.version} <code>{git.path}</code></p>
  {:catch err}
    <p role="alert">{err}</p>
  {/await}
</main>

<style>
  main {
    font-family: system-ui, sans-serif;
    padding: 2rem;
  }
</style>
