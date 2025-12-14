<script lang="ts">
  import { repoStore } from '$lib/stores/repoStore';
  import { open } from '@tauri-apps/plugin-dialog';
  import { invoke } from '@tauri-apps/api/core';
  import type { RepoInfo } from '$lib/types';
  import { Button } from '$lib/components/ui/button';
  import { Badge } from '$lib/components/ui/badge';
  import * as Card from '$lib/components/ui/card';
  import { ScrollArea } from '$lib/components/ui/scroll-area';
  import { Plus, GitBranch, TrendingUp, TrendingDown, X, FolderOpen, GitFork, FolderPlus } from 'lucide-svelte';

  let { repositories, selectedRepo } = $derived($repoStore);
  
  let showAddMenu = $state(false);
  let showCloneDialog = $state(false);
  let showInitDialog = $state(false);
  let cloneUrl = $state('');
  let isProcessing = $state(false);

  async function addRepository() {
    showAddMenu = false;
    console.log('[Add] Opening directory dialog...');
    try {
      const selected = await open({
        directory: true,
        multiple: false,
        title: 'Select Git Repository'
      });
      
      console.log('[Add] Selected directory:', selected);
      
      if (selected && typeof selected === 'string') {
        console.log('[Add] Attempting to add repository:', selected);
        try {
          await repoStore.addRepository(selected);
          console.log('[Add] Repository added successfully!');
        } catch (addError) {
          console.error('[Add] Failed to add repository:', addError);
          alert('Failed to add repository: ' + addError);
        }
      } else {
        console.log('[Add] No directory selected or invalid selection');
      }
    } catch (error) {
      console.error('[Add] Error opening dialog:', error);
      alert('Error opening file dialog: ' + error);
    }
  }

  async function handleClone() {
    if (!cloneUrl.trim()) {
      alert('Please enter a repository URL');
      return;
    }

    isProcessing = true;
    try {
      const selected = await open({
        directory: true,
        multiple: false,
        title: 'Select Clone Destination'
      });

      if (selected && typeof selected === 'string') {
        const result = await invoke<string>('clone_repository_cmd', {
          url: cloneUrl,
          path: selected
        });
        
        // Wait a bit for filesystem to sync
        await new Promise(resolve => setTimeout(resolve, 100));
        
        try {
          await repoStore.addRepository(selected);
          showCloneDialog = false;
          cloneUrl = '';
          alert(result);
        } catch (addError) {
          console.error('Failed to add repository after clone:', addError);
          alert(`Repository cloned but failed to add to list: ${addError}`);
        }
      }
    } catch (error) {
      console.error('Clone error:', error);
      alert('Clone failed: ' + error);
    } finally {
      isProcessing = false;
    }
  }

  async function handleInit() {
    isProcessing = true;
    console.log('[Init] Starting initialization process...');
    try {
      const selected = await open({
        directory: true,
        multiple: false,
        title: 'Select Directory to Initialize'
      });

      console.log('[Init] Selected directory:', selected);

      if (selected && typeof selected === 'string') {
        console.log('[Init] Calling init_repository_cmd with path:', selected);
        const result = await invoke<string>('init_repository_cmd', {
          path: selected
        });
        console.log('[Init] Init command result:', result);
        
        // Wait a bit for filesystem to sync
        await new Promise(resolve => setTimeout(resolve, 500));
        
        console.log('[Init] Attempting to add repository to list...');
        try {
          await repoStore.addRepository(selected);
          console.log('[Init] Repository added successfully!');
          showInitDialog = false;
          alert(result);
        } catch (addError) {
          console.error('[Init] Failed to add repository after init:', addError);
          alert(`Repository initialized at ${selected} but failed to add to list: ${addError}`);
        }
      } else {
        console.log('[Init] No directory selected or invalid selection');
      }
    } catch (error) {
      console.error('[Init] Init error:', error);
      alert('Init failed: ' + error);
    } finally {
      isProcessing = false;
      console.log('[Init] Process complete, isProcessing:', false);
    }
  }

  function selectRepo(repo: RepoInfo) {
    repoStore.selectRepository(repo.path);
  }

  function removeRepo(event: MouseEvent, path: string) {
    event.stopPropagation();
    repoStore.removeRepository(path);
  }

  function getStatusColor(repo: RepoInfo): string {
    if (repo.has_changes) return 'bg-yellow-500';
    return 'bg-green-500';
  }
</script>

<div class="w-80 border-r border-border bg-background flex flex-col h-full">
  <div class="p-4 border-b border-border flex-shrink-0">
    <div class="flex items-center justify-between">
      <h2 class="text-lg font-semibold">Repositories</h2>
      <div class="relative">
        <Button size="sm" onclick={() => showAddMenu = !showAddMenu}>
          <Plus class="h-4 w-4 mr-1" />
          Add
        </Button>
        
        {#if showAddMenu}
          <!-- svelte-ignore a11y_click_events_have_key_events -->
          <!-- svelte-ignore a11y_no_static_element_interactions -->
          <div 
            class="absolute right-0 top-full mt-1 w-48 bg-popover border border-border rounded-md shadow-lg z-50"
            onclick={(e) => e.stopPropagation()}
          >
            <div class="py-1">
              <button
                class="w-full px-4 py-2 text-sm text-left hover:bg-accent flex items-center gap-2"
                onclick={addRepository}
              >
                <FolderOpen class="h-4 w-4" />
                Open Existing
              </button>
              <button
                class="w-full px-4 py-2 text-sm text-left hover:bg-accent flex items-center gap-2"
                onclick={() => { showAddMenu = false; showCloneDialog = true; }}
              >
                <GitFork class="h-4 w-4" />
                Clone Repository
              </button>
              <button
                class="w-full px-4 py-2 text-sm text-left hover:bg-accent flex items-center gap-2"
                onclick={() => { showAddMenu = false; showInitDialog = true; }}
              >
                <FolderPlus class="h-4 w-4" />
                Initialize New
              </button>
            </div>
          </div>
        {/if}
      </div>
    </div>
  </div>

  <ScrollArea class="flex-1">
    <div class="p-3 space-y-2">
      {#if repositories.length === 0}
        <div class="flex flex-col items-center justify-center py-12 px-4 text-center">
          <GitBranch class="h-12 w-12 text-muted-foreground mb-3" />
          <p class="text-sm text-muted-foreground mb-3">No repositories added yet</p>
          <Button onclick={() => showAddMenu = true} variant="outline" size="sm">
            <Plus class="h-4 w-4 mr-1" />
            Add Repository
          </Button>
        </div>
      {:else}
        {#each repositories as repo (repo.path)}
          <Card.Root 
            class="cursor-pointer transition-all hover:bg-accent {selectedRepo === repo.path ? 'ring-2 ring-primary' : ''}"
            onclick={() => selectRepo(repo)}
          >
            <Card.Content class="p-3">
              <div class="flex items-start justify-between gap-2">
                <div class="flex-1 min-w-0">
                  <h3 class="font-medium text-sm truncate mb-1">{repo.name}</h3>
                  <div class="flex items-center gap-1.5 text-xs text-muted-foreground">
                    <GitBranch class="h-3 w-3" />
                    <span class="truncate">{repo.current_branch}</span>
                  </div>
                </div>
      

<!-- Clone Dialog -->
{#if showCloneDialog}
  <!-- svelte-ignore a11y_click_events_have_key_events -->
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div 
    class="fixed inset-0 bg-black/50 flex items-center justify-center z-50" 
    onclick={() => !isProcessing && (showCloneDialog = false)}
  >
    <!-- svelte-ignore a11y_click_events_have_key_events -->
    <!-- svelte-ignore a11y_no_static_element_interactions -->
    <div 
      class="bg-background border border-border rounded-lg p-6 w-96" 
      onclick={(e) => e.stopPropagation()}
    >
      <h3 class="text-lg font-semibold mb-4">Clone Repository</h3>
      <div class="space-y-4">
        <div>
          <label for="clone-url" class="text-sm font-medium block mb-2">Repository URL</label>
          <input
            id="clone-url"
            type="text"
            bind:value={cloneUrl}
            placeholder="https://github.com/user/repo.git"
            class="w-full px-3 py-2 bg-background border border-input rounded-md text-sm"
            disabled={isProcessing}
          />
        </div>
        <div class="flex gap-2 justify-end">
          <Button variant="outline" onclick={() => showCloneDialog = false} disabled={isProcessing}>
            Cancel
          </Button>
          <Button onclick={handleClone} disabled={isProcessing}>
            {isProcessing ? 'Cloning...' : 'Clone'}
          </Button>
        </div>
      </div>
    </div>
  </div>
{/if}

<!-- Init Dialog -->
{#if showInitDialog}
  <!-- svelte-ignore a11y_click_events_have_key_events -->
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div 
    class="fixed inset-0 bg-black/50 flex items-center justify-center z-50" 
    onclick={() => !isProcessing && (showInitDialog = false)}
  >
    <!-- svelte-ignore a11y_click_events_have_key_events -->
    <!-- svelte-ignore a11y_no_static_element_interactions -->
    <div 
      class="bg-background border border-border rounded-lg p-6 w-96" 
      onclick={(e) => e.stopPropagation()}
    >
      <h3 class="text-lg font-semibold mb-4">Initialize Repository</h3>
      <p class="text-sm text-muted-foreground mb-4">
        Select a directory to initialize as a new Git repository
      </p>
      <div class="flex gap-2 justify-end">
        <Button variant="outline" onclick={() => showInitDialog = false} disabled={isProcessing}>
          Cancel
        </Button>
        <Button onclick={handleInit} disabled={isProcessing}>
          {isProcessing ? 'Initializing...' : 'Select & Initialize'}
        </Button>
      </div>
    </div>
  </div>
{/if}          <div class="flex items-center gap-1">
                  <div class="h-2 w-2 rounded-full {getStatusColor(repo)}"></div>
                  <Button 
                    variant="ghost" 
                    size="icon" 
                    class="h-6 w-6"
                    onclick={(e) => removeRepo(e, repo.path)}
                  >
                    <X class="h-3.5 w-3.5" />
                  </Button>
                </div>
              </div>
              
              {#if repo.ahead > 0 || repo.behind > 0}
                <div class="flex items-center gap-2 mt-2 pt-2 border-t border-border">
                  {#if repo.ahead > 0}
                    <Badge variant="outline" class="h-5 text-xs">
                      <TrendingUp class="h-3 w-3 mr-1" />
                      {repo.ahead}
                    </Badge>
                  {/if}
                  {#if repo.behind > 0}
                    <Badge variant="outline" class="h-5 text-xs">
                      <TrendingDown class="h-3 w-3 mr-1" />
                      {repo.behind}
                    </Badge>
                  {/if}
                </div>
              {/if}
            </Card.Content>
          </Card.Root>
        {/each}
      {/if}
    </div>
  </ScrollArea>
</div>

