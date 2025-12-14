import { invoke } from '@tauri-apps/api/core';
import { get, writable } from 'svelte/store';
import type { BranchInfo, CommitInfo, RepoInfo, RepoStatus } from '../types';

interface RepoState {
  repositories: RepoInfo[];
  selectedRepo: string | null;
  repoStatus: RepoStatus | null;
  commits: CommitInfo[];
  branches: BranchInfo[];
  loading: boolean;
  error: string | null;
}

function createRepoStore() {
  const { subscribe, set, update } = writable<RepoState>({
    repositories: [],
    selectedRepo: null,
    repoStatus: null,
    commits: [],
    branches: [],
    loading: false,
    error: null,
  });

  async function refreshRepository(path: string) {
    try {
      update(state => ({ ...state, loading: true, error: null }));
      const repoInfo = await invoke<RepoInfo>('get_repo_info_cmd', { path });
      
      update(state => ({
        ...state,
        repositories: state.repositories.map(r => 
          r.path === path ? repoInfo : r
        ),
        loading: false,
      }));
      
      // If this is the selected repo, refresh its details too
      const currentState = get({ subscribe });
      if (currentState.selectedRepo === path) {
        let status: RepoStatus | null = null;
        let commits: CommitInfo[] = [];
        let branches: BranchInfo[] = [];
        
        try {
          status = await invoke<RepoStatus>('get_repo_status_cmd', { path });
        } catch (error) {
          console.error('[Store] Error refreshing status:', error);
        }
        
        try {
          commits = await invoke<CommitInfo[]>('get_commits_cmd', { path, limit: 50 });
        } catch (error) {
          console.log('[Store] Error refreshing commits (may be expected):', error);
        }
        
        try {
          branches = await invoke<BranchInfo[]>('get_branches_cmd', { path });
        } catch (error) {
          console.log('[Store] Error refreshing branches:', error);
        }
        
        update(state => ({
          ...state,
          repoStatus: status,
          commits,
          branches,
        }));
      }
    } catch (error) {
      console.error('[Store] Error in refreshRepository:', error);
      update(state => ({
        ...state,
        loading: false,
      }));
      // Don't set error state for refresh operations
    }
  }

  return {
    subscribe,
    addRepository: async (path: string) => {
      try {
        update(state => ({ ...state, loading: true, error: null }));
        
        // Check if repository already exists
        const currentState = get({ subscribe });
        if (currentState.repositories.some(r => r.path === path)) {
          console.log('Repository already added:', path);
          update(state => ({ ...state, loading: false }));
          return;
        }
        
        const repoInfo = await invoke<RepoInfo>('get_repo_info_cmd', { path });
        update(state => ({
          ...state,
          repositories: [...state.repositories, repoInfo],
          loading: false,
        }));
        
        // Save to localStorage
        const repos = get({ subscribe }).repositories;
        if (typeof window !== 'undefined') {
          localStorage.setItem('repositories', JSON.stringify(repos.map(r => r.path)));
        }
      } catch (error) {
        console.error('Failed to add repository:', error);
        update(state => ({
          ...state,
          loading: false,
          error: error as string,
        }));
        throw error; // Re-throw so the caller knows it failed
      }
    },
    
    removeRepository: (path: string) => {
      update(state => ({
        ...state,
        repositories: state.repositories.filter(r => r.path !== path),
        selectedRepo: state.selectedRepo === path ? null : state.selectedRepo,
      }));
      
      // Save to localStorage
      const repos = get({ subscribe }).repositories;
      if (typeof window !== 'undefined') {
        localStorage.setItem('repositories', JSON.stringify(repos.map(r => r.path)));
      }
    },
    
    selectRepository: async (path: string) => {
      console.log('[Store] Selecting repository:', path);
      update(state => ({ ...state, selectedRepo: path, loading: true, error: null }));
      
      let status: RepoStatus | null = null;
      let commits: CommitInfo[] = [];
      let branches: BranchInfo[] = [];
      
      try {
        console.log('[Store] Invoking get_repo_status_cmd...');
        status = await invoke<RepoStatus>('get_repo_status_cmd', { path });
        console.log('[Store] Got status:', status);
        console.log('[Store] Status files:', status?.files);
        console.log('[Store] Number of files:', status?.files?.length);
      } catch (error) {
        console.error('[Store] Error getting status:', error);
      }
      
      try {
        console.log('[Store] Invoking get_commits_cmd...');
        commits = await invoke<CommitInfo[]>('get_commits_cmd', { path, limit: 50 });
        console.log('[Store] Got commits:', commits?.length);
      } catch (error) {
        console.log('[Store] Error getting commits (may be expected for new repos):', error);
      }
      
      try {
        console.log('[Store] Invoking get_branches_cmd...');
        branches = await invoke<BranchInfo[]>('get_branches_cmd', { path });
        console.log('[Store] Got branches:', branches?.length);
      } catch (error) {
        console.log('[Store] Error getting branches:', error);
      }
      
      // Update store with whatever data we got
      update(state => ({
        ...state,
        repoStatus: status,
        commits,
        branches,
        loading: false,
      }));
    },
    
    refreshRepository: async (path: string) => {
      await refreshRepository(path);
    },
    
    stageFile: async (path: string, filePath: string) => {
      try {
        await invoke('stage_file_cmd', { path, filePath });
        await refreshRepository(path);
      } catch (error) {
        console.error('[Store] Error staging file:', error);
        update(state => ({ ...state, error: error as string }));
      }
    },
    
    unstageFile: async (path: string, filePath: string) => {
      try {
        await invoke('unstage_file_cmd', { path, filePath });
        await refreshRepository(path);
      } catch (error) {
        console.error('[Store] Error unstaging file:', error);
        update(state => ({ ...state, error: error as string }));
      }
    },
    
    commit: async (path: string, message: string) => {
      try {
        await invoke('commit_cmd', { path, message });
        await refreshRepository(path);
      } catch (error) {
        update(state => ({ ...state, error: error as string }));
      }
    },
    
    pull: async (path: string) => {
      try {
        update(state => ({ ...state, loading: true, error: null }));
        const result = await invoke<string>('pull_cmd', { path });
        await refreshRepository(path);
        update(state => ({ ...state, loading: false }));
        return result;
      } catch (error) {
        update(state => ({
          ...state,
          loading: false,
          error: error as string,
        }));
        throw error;
      }
    },
    
    push: async (path: string) => {
      try {
        update(state => ({ ...state, loading: true, error: null }));
        const result = await invoke<string>('push_cmd', { path });
        await refreshRepository(path);
        update(state => ({ ...state, loading: false }));
        return result;
      } catch (error) {
        update(state => ({
          ...state,
          loading: false,
          error: error as string,
        }));
        throw error;
      }
    },
    
    checkoutBranch: async (path: string, branchName: string) => {
      try {
        update(state => ({ ...state, loading: true, error: null }));
        await invoke('checkout_branch_cmd', { path, branchName });
        await refreshRepository(path);
        update(state => ({ ...state, loading: false }));
      } catch (error) {
        update(state => ({
          ...state,
          loading: false,
          error: error as string,
        }));
      }
    },
    
    loadRepositories: async () => {
      if (typeof window === 'undefined') return;
      
      const saved = localStorage.getItem('repositories');
      if (!saved) return;
      
      const paths = JSON.parse(saved) as string[];
      for (const path of paths) {
        try {
          const repoInfo = await invoke<RepoInfo>('get_repo_info_cmd', { path });
          update(state => ({
            ...state,
            repositories: [...state.repositories, repoInfo],
          }));
        } catch (error) {
          console.error(`Failed to load repository ${path}:`, error);
        }
      }
    },
    
    clearError: () => {
      update(state => ({ ...state, error: null }));
    },
  };
}

export const repoStore = createRepoStore();
