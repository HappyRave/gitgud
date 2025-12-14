<script lang="ts">
  import { onMount } from 'svelte';
  import RepositorySidebar from '$lib/components/RepositorySidebar.svelte';
  import RepositoryDetail from '$lib/components/RepositoryDetail.svelte';
  import Settings from '$lib/components/Settings.svelte';
  import { repoStore } from '$lib/stores/repoStore';
  import { settingsStore } from '$lib/stores/settingsStore';
  import { Button } from '$lib/components/ui/button';
  import { ArrowLeft } from 'lucide-svelte';

  let showSettings = $derived($settingsStore);

  onMount(() => {
    repoStore.loadRepositories();
  });

  function toggleSettings() {
    settingsStore.update(v => !v);
  }
</script>

<div class="flex flex-1 overflow-hidden bg-background text-foreground">
  {#if showSettings}
    <div class="flex-1 flex flex-col">
      <!-- Back button -->
      <div class="p-2 border-b border-border">
        <Button
          variant="ghost"
          size="sm"
          onclick={toggleSettings}
          class="gap-2"
        >
          <ArrowLeft class="w-4 h-4" />
          Back to Repositories
        </Button>
      </div>
      <div class="flex-1 overflow-hidden">
        <Settings />
      </div>
    </div>
  {:else}
    <RepositorySidebar />
    <RepositoryDetail />
  {/if}
</div>

