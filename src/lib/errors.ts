export type FixId = "pull" | "fetch" | "stash" | "settings" | "forcePush" | "abortMerge";

export interface ExplainedError {
  title: string;
  explanation: string;
  concept?: string;
  fix?: { id: FixId; label: string };
  raw: string;
}

interface Rule {
  test: RegExp;
  title: string;
  explanation: string;
  concept?: string;
  fix?: { id: FixId; label: string };
}

const RULES: Rule[] = [
  {
    test: /rejected.*non-fast-forward|fetch first|Updates were rejected because the tip of your current branch is behind/i,
    title: "Remote has newer commits",
    explanation: "The remote branch moved ahead since your last fetch. Pull to bring in those commits before pushing.",
    concept: "Push rejected",
    fix: { id: "pull", label: "Pull now" },
  },
  {
    test: /rejected.*stale info|failed to push some refs/i,
    title: "Push rejected by remote",
    explanation: "The remote branch doesn't match what you last fetched. Fetch updates first or use Push… with lease.",
    concept: "Push rejected",
    fix: { id: "fetch", label: "Fetch remotes" },
  },
  {
    test: /Authentication failed|Invalid username or password|could not read Username|terminal prompts disabled|403 Forbidden/i,
    title: "Authentication failed",
    explanation: "Git couldn't authenticate with the remote. Check your credential manager sign-in or access token.",
    concept: "Credentials",
  },
  {
    test: /Permission denied \(publickey\)|Host key verification failed|Could not read from remote repository/i,
    title: "SSH access failed",
    explanation: "The remote rejected your SSH key or doesn't know this host key yet.",
    concept: "SSH auth",
  },
  {
    test: /index\.lock.*File exists|Unable to create '.*index\.lock'/i,
    title: "Another Git process is holding the repository lock",
    explanation: "A running Git tool (or an interrupted crash) left `.git/index.lock` behind. Wait for it to finish, or remove that stale lock file if nothing is running.",
    concept: "Lock file",
  },
  {
    test: /cannot lock ref|unable to update local ref/i,
    title: "Branch reference lock failed",
    explanation: "A local or remote branch name conflicts with another folder-prefixed branch or stale lock. Fetching with prune usually cleans it up.",
    concept: "Ref lock",
    fix: { id: "fetch", label: "Fetch & prune" },
  },
  {
    test: /Your local changes to the following files would be overwritten/i,
    title: "Uncommitted edits block this switch or merge",
    explanation: "Switching or merging would overwrite files you modified locally. Stash them safely first (undoable later).",
    concept: "Dirty working copy",
    fix: { id: "stash", label: "Stash changes…" },
  },
  {
    test: /untracked working tree files would be overwritten/i,
    title: "Untracked files would be overwritten",
    explanation: "New files on disk share paths with the target branch. Stash (with untracked files) or move them aside first.",
    concept: "Untracked files",
    fix: { id: "stash", label: "Stash with untracked…" },
  },
  {
    test: /Author identity unknown|Please tell me who you are|empty ident name/i,
    title: "Git identity not configured",
    explanation: "Set your name and email so commits record an author.",
    concept: "user.name / user.email",
    fix: { id: "settings", label: "Configure identity…" },
  },
  {
    test: /not a git repository|does not appear to be a git repository/i,
    title: "Not a Git repository",
    explanation: "That folder has no `.git` history or moved away. Check the path or locate it in the Library.",
  },
  {
    test: /HEAD detached|not currently on a branch|You are not currently on a branch/i,
    title: "Detached HEAD cannot push directly",
    explanation: "You're viewing a commit rather than a branch. Create a branch first so new work isn't left unattached.",
    concept: "Detached HEAD",
  },
  {
    test: /Exiting because of an unresolved conflict|you must resolve all conflicts|needs merge|CONFLICT \(/i,
    title: "Unresolved merge conflicts",
    explanation: "Resolve every conflicted file in Working Copy before continuing or switching branches.",
    concept: "Merge conflict",
    fix: { id: "abortMerge", label: "Abort merge" },
  },
  {
    test: /There is no tracking information|has no upstream branch|set-upstream/i,
    title: "No upstream branch configured",
    explanation: "This branch isn't tracking a remote branch yet. Fetch remotes or push and set upstream.",
    concept: "Tracking branch",
    fix: { id: "fetch", label: "Fetch remotes" },
  },
  {
    test: /Could not resolve host|Failed to connect|Connection timed out|unable to access/i,
    title: "Cannot reach remote server",
    explanation: "Check your internet connection, VPN, or remote URL and retry.",
    concept: "Network",
    fix: { id: "fetch", label: "Retry fetch" },
  },
  {
    test: /is not fully merged/i,
    title: "Branch has unmerged commits",
    explanation: "Deleting it normally refuses so work isn't lost; force delete if you genuinely meant to drop it.",
    concept: "Unmerged branch",
  },
  {
    test: /pathspec '.*' did not match|unknown revision|bad revision/i,
    title: "Branch, commit, or file wasn't found",
    explanation: "That reference no longer exists or was mistyped. Fetching or refreshing the repo brings the latest list.",
    fix: { id: "fetch", label: "Fetch remotes" },
  },
  {
    test: /submodule.*modified|submodule.*dirty/i,
    title: "Submodule has local changes",
    explanation: "A nested submodule folder is dirty. Commit, stash, or discard changes inside it before switching.",
    concept: "Submodule",
  },
  {
    test: /git-lfs|smudge error|Pointer file error|LFS:/i,
    title: "Git LFS transfer or pointer error",
    explanation: "Git LFS couldn't download or smudge a large asset. Check network/auth and that git-lfs is installed.",
    concept: "Git LFS",
    fix: { id: "settings", label: "Check diagnostics…" },
  },
  {
    test: /already in progress|MERGE_HEAD exists|CHERRY_PICK_HEAD|REBASE_HEAD/i,
    title: "An operation is already in progress",
    explanation: "Finish or abort the active merge or rebase before starting another operation.",
    fix: { id: "abortMerge", label: "Abort merge" },
  },
  {
    test: /nothing to commit|no changes added to commit/i,
    title: "Nothing staged to commit",
    explanation: "Stage one or more files or hunks in Working Copy before committing.",
  },
  {
    test: /dubious ownership|safe\.directory/i,
    title: "Repository folder ownership mismatch",
    explanation: "Git blocked access because the folder owner differs from your Windows account. Mark it as safe in global git config if you trust it.",
    concept: "safe.directory",
  },
];

export function explainError(raw: string): ExplainedError {
  const trimmed = raw.trim();
  for (const rule of RULES) {
    if (rule.test.test(trimmed)) {
      return {
        title: rule.title,
        explanation: rule.explanation,
        concept: rule.concept,
        fix: rule.fix,
        raw,
      };
    }
  }
  const firstLine = trimmed.split(/\r?\n/).find((l) => l.trim())?.trim() ?? trimmed;
  return {
    title: "Action failed",
    explanation: firstLine.length > 180 ? `${firstLine.slice(0, 177)}…` : firstLine,
    raw,
  };
}
