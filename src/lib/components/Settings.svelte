<script lang="ts">
  import type { GitVersionInfo } from '$lib/types';
  import { invoke } from '@tauri-apps/api/core';
  import { onMount } from 'svelte';
  import * as Card from '$lib/components/ui/card';
  import { Badge } from '$lib/components/ui/badge';
  import { GitBranch, Info, HardDrive, Package } from 'lucide-svelte';

  let gitVersion = $state<GitVersionInfo | null>(null);
  let loading = $state(true);
  let error = $state<string | null>(null);

  onMount(async () => {
    try {
      gitVersion = await invoke<GitVersionInfo>('get_git_version_cmd');
    } catch (err) {
      console.error('Failed to get git version:', err);
      error = String(err);
    } finally {
      loading = false;
    }
  });

  const appVersion = '0.1.0';
</script>

<div class="flex flex-col h-full overflow-hidden">
  <div class="p-4 border-b border-border flex-shrink-0">
    <h1 class="text-2xl font-bold">Settings</h1>
    <p class="text-sm text-muted-foreground">Application and system information</p>
  </div>

  <div class="flex-1 overflow-y-auto">
    <div class="p-4 space-y-4">
      <!-- Application Info -->
      <Card.Root>
        <Card.Header>
          <Card.Title class="flex items-center gap-2">
            <Package class="w-5 h-5" />
            Application
          </Card.Title>
        </Card.Header>
        <Card.Content class="space-y-3">
          <div class="flex justify-between items-center">
            <span class="text-sm text-muted-foreground">Version</span>
            <Badge variant="secondary">{appVersion}</Badge>
          </div>
          <div class="flex justify-between items-center">
            <span class="text-sm text-muted-foreground">Built with</span>
            <span class="text-sm">Tauri + SvelteKit</span>
          </div>
        </Card.Content>
      </Card.Root>

      <!-- Git Info -->
      <Card.Root>
        <Card.Header>
          <Card.Title class="flex items-center gap-2">
            <GitBranch class="w-5 h-5" />
            Git
          </Card.Title>
        </Card.Header>
        <Card.Content>
          {#if loading}
            <p class="text-sm text-muted-foreground">Loading git information...</p>
          {:else if error}
            <p class="text-sm text-destructive">{error}</p>
          {:else if gitVersion}
            <div class="space-y-3">
              <div class="flex justify-between items-center">
                <span class="text-sm text-muted-foreground">Version</span>
                <Badge variant="secondary">{gitVersion.version}</Badge>
              </div>
              <div class="flex flex-col gap-1">
                <span class="text-sm text-muted-foreground">Path</span>
                <code class="text-xs bg-muted p-2 rounded break-all">{gitVersion.path}</code>
              </div>
            </div>
          {:else}
            <p class="text-sm text-muted-foreground">No git information available</p>
          {/if}
        </Card.Content>
      </Card.Root>

      <!-- System Info -->
      <Card.Root>
        <Card.Header>
          <Card.Title class="flex items-center gap-2">
            <HardDrive class="w-5 h-5" />
            System
          </Card.Title>
        </Card.Header>
        <Card.Content class="space-y-3">
          <div class="flex justify-between items-center">
            <span class="text-sm text-muted-foreground">Platform</span>
            <Badge variant="secondary">{navigator.platform}</Badge>
          </div>
          <div class="flex justify-between items-center">
            <span class="text-sm text-muted-foreground">User Agent</span>
            <span class="text-xs text-muted-foreground max-w-[200px] truncate">{navigator.userAgent}</span>
          </div>
        </Card.Content>
      </Card.Root>

      <!-- About -->
      <Card.Root>
        <Card.Header>
          <Card.Title class="flex items-center gap-2">
            <Info class="w-5 h-5" />
            About
          </Card.Title>
        </Card.Header>
        <Card.Content>
          <p class="text-sm text-muted-foreground">
            GitGud is a modern Git client built with Tauri and SvelteKit. Manage your repositories with a clean, native interface.
          </p>
        </Card.Content>
      </Card.Root>
    </div>
  </div>
</div>
