<script lang="ts">
  import { repoStore } from '$lib/stores/repoStore';
  import type { FileStatus, FileDiff } from '$lib/types';
  import { invoke } from '@tauri-apps/api/core';
  import { Store } from '@tauri-apps/plugin-store';
  import { Button } from '$lib/components/ui/button';
  import { Badge } from '$lib/components/ui/badge';
  import * as Card from '$lib/components/ui/card';
  import * as Tabs from '$lib/components/ui/tabs';
  import { ScrollArea } from '$lib/components/ui/scroll-area';
  import { Separator } from '$lib/components/ui/separator';
  import { 
    GitBranch, TrendingUp, TrendingDown, RefreshCw, Download, Upload, 
    FileIcon, Plus, Minus, GitCommit, Clock, User, Hash, AlertCircle,
    CheckCircle, FileEdit, FileX, FilePlus, X
  } from 'lucide-svelte';

  let { selectedRepo, repoStatus, commits, branches, loading, error } = $derived($repoStore);
  let commitMessage = $state('');
  let showCommitDialog = $state(false);
  let showRemoteDialog = $state(false);
  let showCredentialsDialog = $state(false);
  let remoteName = $state('origin');
  let remoteUrl = $state('');
  let gitUsername = $state('');
  let gitPassword = $state('');
  let credentialStore: Store | null = $state(null);
  let selectedFileForDiff: FileStatus | null = $state(null);
  let fileDiff: FileDiff | null = $state(null);
  let loadingDiff = $state(false);
  
  // Initialize credential store
  $effect(() => {
    Store.load('credentials.json').then(store => {
      credentialStore = store;
      // Load saved credentials
      store.get('username').then((username: string | null) => {
        if (username) gitUsername = username;
      });
      store.get('password').then((password: string | null) => {
        if (password) gitPassword = password;
      });
    }).catch(err => {
      console.error('Failed to load credential store:', err);
    });
  });
  let activeTab: 'changes' | 'history' | 'branches' = $state('changes');
  let selectedFiles = $state<Set<string>>(new Set());
  let lastSelectedIndex = $state<number>(-1);
  let previousRepo = $state<string | null>(null);
  
  // Watch for repository changes and clear selection only when repo actually changes
  $effect(() => {
    if (selectedRepo !== previousRepo) {
      console.log('[Detail] Selected repo changed from', previousRepo, 'to', selectedRepo);
      selectedFiles = new Set();
      lastSelectedIndex = -1;
      selectedFileForDiff = null;
      fileDiff = null;
      previousRepo = selectedRepo;
    }
  });

  function handleFileClick(file: FileStatus, index: number, event: MouseEvent) {
    console.log('[FileClick]', { file: file.path, index, ctrl: event.ctrlKey, shift: event.shiftKey });
    
    // Prevent text selection when shift-clicking
    if (event.shiftKey) {
      event.preventDefault();
    }
    
    if (event.ctrlKey || event.metaKey) {
      // Ctrl/Cmd: toggle selection
      const newSet = new Set(selectedFiles);
      if (newSet.has(file.path)) {
        newSet.delete(file.path);
        console.log('[FileClick] Deselected:', file.path);
      } else {
        newSet.add(file.path);
        console.log('[FileClick] Selected:', file.path);
      }
      selectedFiles = newSet;
      console.log('[FileClick] New selection size:', selectedFiles.size);
      lastSelectedIndex = index;
    } else if (event.shiftKey && lastSelectedIndex >= 0) {
      // Shift: range selection
      const newSet = new Set(selectedFiles);
      const start = Math.min(lastSelectedIndex, index);
      const end = Math.max(lastSelectedIndex, index);
      const files = unstagedFiles;
      console.log('[FileClick] Shift range:', start, 'to', end);
      for (let i = start; i <= end && i < files.length; i++) {
        newSet.add(files[i].path);
      }
      selectedFiles = newSet;
      console.log('[FileClick] New selection size:', selectedFiles.size);
    } else {
      // Normal click: single selection and show diff
      console.log('[FileClick] Single selection:', file.path);
      selectedFiles = new Set([file.path]);
      lastSelectedIndex = index;
      loadFileDiff(file);
    }
  }

  async function loadFileDiff(file: FileStatus) {
    if (!selectedRepo) return;
    loadingDiff = true;
    selectedFileForDiff = file;
    try {
      fileDiff = await invoke<FileDiff>('get_file_diff_cmd', {
        path: selectedRepo,
        filePath: file.path,
        staged: file.staged
      });
    } catch (err) {
      console.error('Failed to load diff:', err);
      fileDiff = null;
    } finally {
      loadingDiff = false;
    }
  }

  function closeDiffPane() {
    selectedFileForDiff = null;
    fileDiff = null;
  }

  async function stageFile(file: FileStatus) {
    if (selectedRepo) {
      await repoStore.stageFile(selectedRepo, file.path);
      const newSet = new Set(selectedFiles);
      newSet.delete(file.path);
      selectedFiles = newSet;
      // Clear diff if showing this file
      if (selectedFileForDiff?.path === file.path) {
        selectedFileForDiff = null;
        fileDiff = null;
      }
    }
  }

  async function unstageFile(file: FileStatus) {
    if (selectedRepo) {
      await repoStore.unstageFile(selectedRepo, file.path);
      // Clear diff if showing this file
      if (selectedFileForDiff?.path === file.path) {
        selectedFileForDiff = null;
        fileDiff = null;
      }
    }
  }

  async function stageAllFiles() {
    if (selectedRepo) {
      for (const file of unstagedFiles) {
        await repoStore.stageFile(selectedRepo, file.path);
      }
      selectedFiles = new Set();
      selectedFileForDiff = null;
      fileDiff = null;
    }
  }

  async function stageSelectedFiles() {
    if (selectedRepo && selectedFiles.size > 0) {
      for (const filePath of selectedFiles) {
        await repoStore.stageFile(selectedRepo, filePath);
      }
      selectedFiles = new Set();
      selectedFileForDiff = null;
      fileDiff = null;
    }
  }

  async function handleCommit() {
    if (selectedRepo && commitMessage.trim()) {
      await repoStore.commit(selectedRepo, commitMessage);
      commitMessage = '';
      showCommitDialog = false;
      selectedFileForDiff = null;
      fileDiff = null;
    }
  }

  async function handleAddRemote() {
    if (selectedRepo && remoteName.trim() && remoteUrl.trim()) {
      try {
        await invoke('add_remote_cmd', { 
          path: selectedRepo, 
          name: remoteName.trim(), 
          url: remoteUrl.trim() 
        });
        showRemoteDialog = false;
        remoteName = 'origin';
        remoteUrl = '';
        // Reload the repository data to reflect the new remote
        await repoStore.selectRepository(selectedRepo);
      } catch (error) {
        console.error('Failed to add remote:', error);
        // Show error in the UI instead of alert
      }
    }
  }

  async function handlePull() {
    if (selectedRepo) {
      try {
        const result = await repoStore.pull(selectedRepo);
        alert(result);
      } catch (err) {
        alert('Pull failed: ' + err);
      }
    }
  }

  async function handlePush() {
    if (selectedRepo) {
      try {
        // Try with saved credentials if available
        if (gitUsername && gitPassword) {
          await invoke('push_with_credentials_cmd', {
            path: selectedRepo,
            username: gitUsername,
            password: gitPassword
          });
          await repoStore.selectRepository(selectedRepo);
        } else {
          // Otherwise try without credentials (works for SSH)
          const result = await repoStore.push(selectedRepo);
          console.log('Push successful:', result);
        }
        selectedFileForDiff = null;
        fileDiff = null;
      } catch (err) {
        const errorStr = String(err);
        // If authentication is required, show credentials dialog
        if (errorStr.includes('Authentication') || errorStr.includes('401') || errorStr.includes('403')) {
          showCredentialsDialog = true;
        } else {
          console.error('Push failed:', err);
        }
      }
    }
  }

  async function handlePushWithCredentials() {
    if (selectedRepo && gitUsername.trim() && gitPassword.trim()) {
      try {
        await invoke('push_with_credentials_cmd', {
          path: selectedRepo,
          username: gitUsername.trim(),
          password: gitPassword.trim()
        });
        
        // Save credentials for future use
        if (credentialStore) {
          await credentialStore.set('username', gitUsername.trim());
          await credentialStore.set('password', gitPassword.trim());
          await credentialStore.save();
        }
        
        showCredentialsDialog = false;
        await repoStore.selectRepository(selectedRepo);
        selectedFileForDiff = null;
        fileDiff = null;
      } catch (error) {
        console.error('Push with credentials failed:', error);
      }
    }
  }

  async function handleCheckoutBranch(branchName: string) {
    if (selectedRepo) {
      const cleanName = branchName.replace('origin/', '');
      await repoStore.checkoutBranch(selectedRepo, cleanName);
    }
  }

  async function refresh() {
    if (selectedRepo) {
      await repoStore.refreshRepository(selectedRepo);
    }
  }

  function formatDate(timestamp: number): string {
    return new Date(timestamp * 1000).toLocaleString();
  }

  function getStatusIcon(status: string) {
    switch (status) {
      case 'new':
      case 'untracked':
        return FilePlus;
      case 'modified':
        return FileEdit;
      case 'deleted':
        return FileX;
      default:
        return FileIcon;
    }
  }

  function getStatusVariant(status: string): 'default' | 'secondary' | 'destructive' {
    switch (status) {
      case 'new':
      case 'untracked':
        return 'default';
      case 'modified':
        return 'secondary';
      case 'deleted':
        return 'destructive';
      default:
        return 'secondary';
    }
  }

  let stagedFiles = $derived(repoStatus?.files.filter(f => f.staged) ?? []);
  let unstagedFiles = $derived(repoStatus?.files.filter(f => !f.staged) ?? []);
  let hasNoCommits = $derived(commits.length === 0);
  let hasNoRemote = $derived(!repoStatus?.has_remote);
</script>

<div class="flex-1 flex flex-col bg-background h-full overflow-hidden">
  {#if !selectedRepo}
    <div class="flex items-center justify-center h-full">
      <div class="text-center">
        <GitBranch class="h-16 w-16 mx-auto text-muted-foreground mb-4" />
        <h2 class="text-2xl font-semibold mb-2">No Repository Selected</h2>
        <p class="text-muted-foreground">Select a repository from the sidebar or add a new one</p>
      </div>
    </div>
  {:else}
    <!-- Header -->
    <div class="border-b border-border p-4 flex-shrink-0">
      <div class="flex items-center justify-between">
        <div>
          <h1 class="text-2xl font-bold mb-2">Repository Status</h1>
          {#if repoStatus}
            <div class="flex items-center gap-3">
              <div class="flex items-center gap-1.5">
                <GitBranch class="h-4 w-4 text-muted-foreground" />
                <span class="font-medium">{repoStatus.current_branch}</span>
              </div>
              {#if repoStatus.ahead > 0}
                <Badge variant="outline">
                  <TrendingUp class="h-3 w-3 mr-1" />
                  {repoStatus.ahead} ahead
                </Badge>
              {/if}
              {#if repoStatus.behind > 0}
                <Badge variant="outline">
                  <TrendingDown class="h-3 w-3 mr-1" />
                  {repoStatus.behind} behind
                </Badge>
              {/if}
            </div>
          {/if}
        </div>
        <div class="flex gap-2">
          <Button variant="outline" size="sm" onclick={refresh} disabled={loading}>
            <RefreshCw class="h-4 w-4 mr-1 {loading ? 'animate-spin' : ''}" />
            Refresh
          </Button>
          <Button variant="outline" size="sm" onclick={handlePull} disabled={loading}>
            <Download class="h-4 w-4 mr-1" />
            Pull
          </Button>
          <Button variant="outline" size="sm" onclick={handlePush} disabled={loading}>
            <Upload class="h-4 w-4 mr-1" />
            Push
          </Button>
          <Button size="sm" onclick={() => showCommitDialog = true} disabled={stagedFiles.length === 0}>
            <GitCommit class="h-4 w-4 mr-1" />
            Commit
          </Button>
        </div>
      </div>
    </div>

    {#if error}
      <div class="bg-destructive/10 border-l-4 border-destructive px-4 py-3 flex items-center justify-between flex-shrink-0">
        <div class="flex items-center gap-2">
          <AlertCircle class="h-4 w-4 text-destructive" />
          <span class="text-sm">{error}</span>
        </div>
        <Button variant="ghost" size="icon" onclick={() => repoStore.clearError()}>
          <span class="text-lg">×</span>
        </Button>
      </div>
    {/if}

    {#if hasNoCommits && !error}
      <div class="bg-blue-500/10 border-l-4 border-blue-500 px-4 py-3 flex items-center gap-2 flex-shrink-0">
        <AlertCircle class="h-4 w-4 text-blue-500" />
        <span class="text-sm">New repository - Make your first commit to get started</span>
      </div>
    {:else if hasNoRemote && !hasNoCommits && !error}
      <div class="bg-yellow-500/10 border-l-4 border-yellow-500 px-4 py-3 flex items-center justify-between flex-shrink-0">
        <div class="flex items-center gap-2">
          <AlertCircle class="h-4 w-4 text-yellow-500" />
          <span class="text-sm">No remote configured - Add a remote to push/pull changes</span>
        </div>
        <Button size="sm" variant="outline" onclick={() => showRemoteDialog = true}>
          Add Remote
        </Button>
      </div>
    {/if}

    <!-- Tabs -->
    <div class="flex-1 flex overflow-hidden">
      <Tabs.Root value={activeTab} onValueChange={(v) => activeTab = (v as typeof activeTab) || 'changes'} class="flex-1 flex flex-col min-h-0">
        <Tabs.List class="w-full justify-start border-b px-4 flex-shrink-0">
          <Tabs.Trigger value="changes">
            Changes {#if repoStatus?.files.length}{`(${repoStatus.files.length})`}{/if}
          </Tabs.Trigger>
          <Tabs.Trigger value="history">
            History {#if commits.length}{`(${commits.length})`}{/if}
          </Tabs.Trigger>
          <Tabs.Trigger value="branches">
            Branches {#if branches.length}{`(${branches.length})`}{/if}
          </Tabs.Trigger>
        </Tabs.List>

        <ScrollArea class="flex-1">
          <div class="p-4">
          <!-- Changes Tab -->
          <Tabs.Content value="changes" class="mt-0">
            {#if stagedFiles.length === 0 && unstagedFiles.length === 0}
              <Card.Root>
                <Card.Content class="pt-6">
                  <div class="text-center py-8">
                    <CheckCircle class="h-12 w-12 mx-auto text-green-600 mb-4" />
                    <h3 class="text-lg font-semibold mb-2">Working Tree Clean</h3>
                    <p class="text-muted-foreground">No changes to commit</p>
                  </div>
                </Card.Content>
              </Card.Root>
            {:else}
              <div class="space-y-4">
                <!-- Staged Files -->
                {#if stagedFiles.length > 0}
                  <Card.Root>
                    <Card.Header>
                      <Card.Title class="flex items-center gap-2">
                        <CheckCircle class="h-5 w-5 text-green-600" />
                        Staged Changes ({stagedFiles.length})
                      </Card.Title>
                      <Card.Description>Ready to commit</Card.Description>
                    </Card.Header>
                    <Card.Content>
                      <div class="space-y-2">
                        {#each stagedFiles as file}
                          {@const StatusIcon = getStatusIcon(file.status)}
                          <div class="flex items-center justify-between p-3 rounded-lg border bg-card hover:bg-accent/50 transition-colors">
                            <div class="flex items-center gap-3">
                              <StatusIcon class="h-4 w-4 text-muted-foreground" />
                              <span class="font-mono text-sm">{file.path}</span>
                              <Badge variant={getStatusVariant(file.status)}>{file.status}</Badge>
                            </div>
                            <Button 
                              variant="ghost" 
                              size="icon"
                              onclick={() => unstageFile(file)}
                            >
                              <Minus class="h-4 w-4" />
                            </Button>
                          </div>
                        {/each}
                      </div>
                    </Card.Content>
                  </Card.Root>
                {/if}

                <!-- Unstaged Files -->
                {#if unstagedFiles.length > 0}
                  <Card.Root>
                    <Card.Header>
                      <div class="flex items-center justify-between">
                        <div>
                          <Card.Title class="flex items-center gap-2">
                            <FileEdit class="h-5 w-5" />
                            Unstaged Changes ({unstagedFiles.length})
                          </Card.Title>
                          <Card.Description>Not yet staged for commit</Card.Description>
                        </div>
                        <div class="flex gap-2">
                          {#if selectedFiles.size > 0}
                            <Button size="sm" onclick={stageSelectedFiles}>
                              <Plus class="h-4 w-4 mr-1" />
                              Stage Selected ({selectedFiles.size})
                            </Button>
                          {/if}
                          <Button size="sm" variant="outline" onclick={stageAllFiles}>
                            <Plus class="h-4 w-4 mr-1" />
                            Stage All
                          </Button>
                        </div>
                      </div>
                    </Card.Header>
                    <Card.Content>
                      <ScrollArea class="h-[400px]">
                        <div class="space-y-2 pr-4" onclick={(e) => {
                          // Clear selection when clicking on empty space
                          if (e.target === e.currentTarget) {
                            selectedFiles = new Set();
                            console.log('[FileClick] Cleared selection');
                          }
                        }}>
                        {#each unstagedFiles as file, index}
                          {@const StatusIcon = getStatusIcon(file.status)}
                          {@const isSelected = selectedFiles.has(file.path)}
                          <div 
                            class="flex items-center justify-between p-3 rounded-lg border transition-colors cursor-pointer {isSelected ? 'bg-primary/20 border-primary/50' : 'bg-card hover:bg-accent/50'}"
                            onclick={(e) => handleFileClick(file, index, e)}
                            onmousedown={(e) => { if (e.shiftKey) e.preventDefault(); }}
                            role="button"
                            tabindex="0"
                          >
                            <div class="flex items-center gap-3">
                              <StatusIcon class="h-4 w-4 text-muted-foreground" />
                              <span class="font-mono text-sm">{file.path}</span>
                              <Badge variant={getStatusVariant(file.status)}>{file.status}</Badge>
                            </div>
                            <Button 
                              variant="ghost" 
                              size="icon"
                              onclick={(e) => { e.stopPropagation(); stageFile(file); }}
                            >
                              <Plus class="h-4 w-4" />
                            </Button>
                          </div>
                        {/each}
                        </div>
                      </ScrollArea>
                    </Card.Content>
                  </Card.Root>
                {/if}
              </div>
            {/if}
          </Tabs.Content>

          <!-- History Tab -->
          <Tabs.Content value="history" class="mt-0">
            {#if commits.length === 0}
              <Card.Root>
                <Card.Content class="pt-6">
                  <div class="text-center py-8">
                    <Clock class="h-12 w-12 mx-auto text-muted-foreground mb-4" />
                    <h3 class="text-lg font-semibold mb-2">No Commits Yet</h3>
                    <p class="text-muted-foreground">Make your first commit to see history</p>
                  </div>
                </Card.Content>
              </Card.Root>
            {:else}
              <div class="space-y-3">
                {#each commits as commit}
                  <Card.Root>
                    <Card.Content class="pt-6">
                      <div class="flex gap-4">
                        <div class="flex flex-col items-center">
                          <div class="h-8 w-8 rounded-full bg-primary/10 flex items-center justify-center">
                            <GitCommit class="h-4 w-4 text-primary" />
                          </div>
                          {#if commit !== commits[commits.length - 1]}
                            <div class="flex-1 w-px bg-border mt-2"></div>
                          {/if}
                        </div>
                        <div class="flex-1 pb-4">
                          <div class="flex items-start justify-between mb-2">
                            <h4 class="font-semibold">{commit.message}</h4>
                            <Badge variant="outline" class="ml-2">
                              <Hash class="h-3 w-3 mr-1" />
                              {commit.id.substring(0, 7)}
                            </Badge>
                          </div>
                          <div class="flex items-center gap-4 text-sm text-muted-foreground">
                            <div class="flex items-center gap-1">
                              <User class="h-3 w-3" />
                              <span>{commit.author}</span>
                            </div>
                            <div class="flex items-center gap-1">
                              <Clock class="h-3 w-3" />
                              <span>{formatDate(commit.timestamp)}</span>
                            </div>
                          </div>
                        </div>
                      </div>
                    </Card.Content>
                  </Card.Root>
                {/each}
              </div>
            {/if}
          </Tabs.Content>

          <!-- Branches Tab -->
          <Tabs.Content value="branches" class="mt-0">
            {#if branches.length === 0}
              <Card.Root>
                <Card.Content class="pt-6">
                  <div class="text-center py-8">
                    <GitBranch class="h-12 w-12 mx-auto text-muted-foreground mb-4" />
                    <h3 class="text-lg font-semibold mb-2">No Branches Found</h3>
                    <p class="text-muted-foreground">Create branches to manage development</p>
                  </div>
                </Card.Content>
              </Card.Root>
            {:else}
              <div class="space-y-2">
                {#each branches as branch}
                  <Card.Root>
                    <Card.Content class="py-4">
                      <div class="flex items-center justify-between">
                        <div class="flex items-center gap-3">
                          <GitBranch class="h-4 w-4 text-muted-foreground" />
                          <span class="font-mono">{branch.name}</span>
                          {#if branch.is_head}
                            <Badge>Current</Badge>
                          {/if}
                          {#if branch.is_remote}
                            <Badge variant="outline">Remote</Badge>
                          {/if}
                        </div>
                        {#if !branch.is_head}
                          <Button 
                            variant="outline" 
                            size="sm"
                            onclick={() => handleCheckoutBranch(branch.name)}
                          >
                            Checkout
                          </Button>
                        {/if}
                      </div>
                    </Card.Content>
                  </Card.Root>
                {/each}
              </div>
            {/if}
          </Tabs.Content>
        </div>
      </ScrollArea>
    </Tabs.Root>

    <!-- Diff Pane -->
    {#if selectedFileForDiff && fileDiff}
      <div class="w-1/2 border-l border-border flex flex-col overflow-hidden">
        <div class="p-3 border-b border-border flex items-center justify-between bg-muted/30 flex-shrink-0">
          <div class="flex items-center gap-2">
            <FileEdit class="h-4 w-4" />
            <span class="font-medium text-sm">{selectedFileForDiff.path}</span>
            <Badge variant={selectedFileForDiff.staged ? "default" : "secondary"} class="text-xs">
              {selectedFileForDiff.status}
            </Badge>
          </div>
          <Button variant="ghost" size="icon" class="h-7 w-7" onclick={closeDiffPane}>
            <X class="h-4 w-4" />
          </Button>
        </div>
        
        <div class="flex-1 overflow-y-auto bg-background">
          {#if loadingDiff}
            <div class="flex items-center justify-center h-full">
              <span class="text-sm text-muted-foreground">Loading diff...</span>
            </div>
          {:else if fileDiff.lines && fileDiff.lines.length > 0}
            <div class="font-mono text-xs">
              {#each fileDiff.lines as line}
                {#if line.line_type === 'hunk'}
                  <div class="bg-muted/50 px-2 py-1 text-muted-foreground border-y border-border sticky top-0">
                    {line.content}
                  </div>
                {:else}
                  <div class="flex hover:bg-muted/30 {
                    line.line_type === 'add' ? 'bg-green-50 dark:bg-green-950/30' :
                    line.line_type === 'delete' ? 'bg-red-50 dark:bg-red-950/30' :
                    ''
                  }">
                    <!-- Old line number -->
                    <div class="w-12 flex-shrink-0 text-right px-2 py-0.5 select-none border-r border-border/50 text-muted-foreground/60 {
                      line.line_type === 'add' ? 'bg-green-100 dark:bg-green-950/50' :
                      line.line_type === 'delete' ? 'bg-red-100 dark:bg-red-950/50' :
                      'bg-muted/20'
                    }">
                      {line.old_line_num ?? ''}
                    </div>
                    <!-- New line number -->
                    <div class="w-12 flex-shrink-0 text-right px-2 py-0.5 select-none border-r border-border text-muted-foreground/60 {
                      line.line_type === 'add' ? 'bg-green-100 dark:bg-green-950/50' :
                      line.line_type === 'delete' ? 'bg-red-100 dark:bg-red-950/50' :
                      'bg-muted/20'
                    }">
                      {line.new_line_num ?? ''}
                    </div>
                    <!-- Content -->
                    <div class="flex-1 px-3 py-0.5 whitespace-pre-wrap break-all {
                      line.line_type === 'add' ? 'text-green-700 dark:text-green-400' :
                      line.line_type === 'delete' ? 'text-red-700 dark:text-red-400' :
                      'text-foreground'
                    }">
                      {line.content}
                    </div>
                  </div>
                {/if}
              {/each}
            </div>
          {:else}
            <div class="flex items-center justify-center h-full">
              <span class="text-sm text-muted-foreground">No changes to display</span>
            </div>
          {/if}
        </div>
      </div>
    {/if}
  </div>
  {/if}
</div>

<!-- Commit Dialog -->
{#if showCommitDialog}
  <div 
    class="fixed inset-0 bg-black/50 flex items-center justify-center z-50" 
    onclick={() => showCommitDialog = false}
    onkeydown={(e) => e.key === 'Escape' && (showCommitDialog = false)}
    role="dialog"
    aria-modal="true"
    aria-labelledby="commit-dialog-title"
  >
    <Card.Root class="w-full max-w-md" onclick={(e) => e.stopPropagation()}>
      <Card.Header>
        <Card.Title id="commit-dialog-title">Commit Changes</Card.Title>
        <Card.Description>
          Committing {stagedFiles.length} file{stagedFiles.length !== 1 ? 's' : ''}
        </Card.Description>
      </Card.Header>
      <Card.Content>
        <div class="space-y-4">
          <div>
            <label for="commit-message" class="text-sm font-medium mb-2 block">Commit Message</label>
            <textarea
              id="commit-message"
              bind:value={commitMessage}
              class="w-full min-h-[100px] p-3 rounded-md border bg-background resize-none"
              placeholder="Describe your changes..."
            ></textarea>
          </div>
          <Separator />
          <div class="space-y-2 max-h-40 overflow-y-auto">
            <p class="text-sm font-medium">Files to commit:</p>
            {#each stagedFiles as file}
              <div class="text-sm text-muted-foreground font-mono">{file.path}</div>
            {/each}
          </div>
        </div>
      </Card.Content>
      <Card.Footer class="flex gap-2">
        <Button 
          variant="outline" 
          class="flex-1"
          onclick={() => showCommitDialog = false}
        >
          Cancel
        </Button>
        <Button 
          class="flex-1"
          onclick={handleCommit}
          disabled={!commitMessage.trim()}
        >
          Commit
        </Button>
      </Card.Footer>
    </Card.Root>
  </div>
{/if}

<!-- Add Remote Dialog -->
{#if showRemoteDialog}
  <div 
    class="fixed inset-0 bg-black/50 flex items-center justify-center z-50"
    onclick={() => showRemoteDialog = false}
    role="dialog"
    aria-labelledby="remote-dialog-title"
  >
    <Card.Root class="w-full max-w-md" onclick={(e) => e.stopPropagation()}>
      <Card.Header>
        <Card.Title id="remote-dialog-title">Add Remote</Card.Title>
        <Card.Description>
          Configure a remote repository to push/pull changes
        </Card.Description>
      </Card.Header>
      <Card.Content>
        <div class="space-y-4">
          <div>
            <label for="remote-name" class="text-sm font-medium mb-2 block">Remote Name</label>
            <input
              id="remote-name"
              type="text"
              bind:value={remoteName}
              class="w-full p-2 rounded-md border bg-background"
              placeholder="origin"
            />
          </div>
          <div>
            <label for="remote-url" class="text-sm font-medium mb-2 block">Remote URL</label>
            <input
              id="remote-url"
              type="text"
              bind:value={remoteUrl}
              class="w-full p-2 rounded-md border bg-background"
              placeholder="https://github.com/user/repo.git"
            />
          </div>
        </div>
      </Card.Content>
      <Card.Footer class="flex gap-2">
        <Button 
          variant="outline" 
          class="flex-1"
          onclick={() => showRemoteDialog = false}
        >
          Cancel
        </Button>
        <Button 
          class="flex-1"
          onclick={handleAddRemote}
          disabled={!remoteName.trim() || !remoteUrl.trim()}
        >
          Add Remote
        </Button>
      </Card.Footer>
    </Card.Root>
  </div>
{/if}

<!-- Credentials Dialog -->
{#if showCredentialsDialog}
  <div 
    class="fixed inset-0 bg-black/50 flex items-center justify-center z-50"
    onclick={() => showCredentialsDialog = false}
    role="dialog"
    aria-labelledby="credentials-dialog-title"
  >
    <Card.Root class="w-full max-w-md" onclick={(e) => e.stopPropagation()}>
      <Card.Header>
        <Card.Title id="credentials-dialog-title">Git Credentials Required</Card.Title>
        <Card.Description>
          Enter your GitHub username and Personal Access Token (PAT)
        </Card.Description>
      </Card.Header>
      <Card.Content>
        <div class="space-y-4">
          <div>
            <label for="git-username" class="text-sm font-medium mb-2 block">GitHub Username</label>
            <input
              id="git-username"
              type="text"
              bind:value={gitUsername}
              class="w-full p-2 rounded-md border bg-background"
              placeholder="your-username"
            />
          </div>
          <div>
            <label for="git-password" class="text-sm font-medium mb-2 block">Personal Access Token</label>
            <input
              id="git-password"
              type="password"
              bind:value={gitPassword}
              class="w-full p-2 rounded-md border bg-background"
              placeholder="ghp_xxxxxxxxxxxxx"
            />
            <p class="text-xs text-muted-foreground mt-1">
              Create a PAT at: Settings → Developer settings → Personal access tokens
            </p>
          </div>
        </div>
      </Card.Content>
      <Card.Footer class="flex gap-2">
        <Button 
          variant="outline" 
          class="flex-1"
          onclick={() => showCredentialsDialog = false}
        >
          Cancel
        </Button>
        <Button 
          class="flex-1"
          onclick={handlePushWithCredentials}
          disabled={!gitUsername.trim() || !gitPassword.trim()}
        >
          Push
        </Button>
      </Card.Footer>
    </Card.Root>
  </div>
{/if}

