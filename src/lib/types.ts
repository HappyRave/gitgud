export interface RepoInfo {
  path: string;
  name: string;
  current_branch: string;
  has_changes: boolean;
  ahead: number;
  behind: number;
  has_remote: boolean;
}

export interface FileStatus {
  path: string;
  status: string;
  staged: boolean;
}

export interface CommitInfo {
  id: string;
  message: string;
  author: string;
  timestamp: number;
  short_id: string;
}

export interface RepoStatus {
  current_branch: string;
  files: FileStatus[];
  ahead: number;
  behind: number;
  has_changes: boolean;
  has_remote: boolean;
}

export interface BranchInfo {
  name: string;
  is_current: boolean;
  is_remote: boolean;
}

export interface GitVersionInfo {
  version: string;
  path: string;
}
