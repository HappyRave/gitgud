use git2::{BranchType, Repository, Status, StatusOptions};
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::process::Command;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct GitVersionInfo {
    pub version: String,
    pub path: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct RepoInfo {
    pub path: String,
    pub name: String,
    pub current_branch: String,
    pub has_changes: bool,
    pub ahead: usize,
    pub behind: usize,
    pub has_remote: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct FileStatus {
    pub path: String,
    pub status: String,
    pub staged: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct CommitInfo {
    pub id: String,
    pub message: String,
    pub author: String,
    pub timestamp: i64,
    pub short_id: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct RepoStatus {
    pub current_branch: String,
    pub files: Vec<FileStatus>,
    pub ahead: usize,
    pub behind: usize,
    pub has_changes: bool,
    pub has_remote: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct BranchInfo {
    pub name: String,
    pub is_current: bool,
    pub is_remote: bool,
}

pub fn get_git_version() -> Result<GitVersionInfo, String> {
    let output = Command::new("git")
        .arg("--version")
        .output()
        .map_err(|e| format!("Failed to execute git command: {}", e))?;

    if !output.status.success() {
        return Err("Git command failed".to_string());
    }

    let version = String::from_utf8_lossy(&output.stdout).trim().to_string();

    // Get git path
    let path_output = if cfg!(windows) {
        Command::new("where").arg("git").output()
    } else {
        Command::new("which").arg("git").output()
    };

    let path = match path_output {
        Ok(output) if output.status.success() => String::from_utf8_lossy(&output.stdout)
            .lines()
            .next()
            .unwrap_or("Unknown")
            .to_string(),
        _ => "Unknown".to_string(),
    };

    Ok(GitVersionInfo { version, path })
}

pub fn get_repo_info(path: &str) -> Result<RepoInfo, String> {
    println!("[Rust] Getting repo info for: {}", path);
    let repo = match Repository::open(path) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("[Rust] Failed to open repository at {}: {}", path, e);
            return Err(e.to_string());
        }
    };

    // Handle both normal branches and unborn branches (newly initialized repos)
    let current_branch = match repo.head() {
        Ok(head) => head.shorthand().unwrap_or("HEAD").to_string(),
        Err(e) if e.code() == git2::ErrorCode::UnbornBranch => {
            // Repository is newly initialized with no commits
            println!("[Rust] Unborn branch detected, using 'main'");
            "main".to_string()
        }
        Err(e) => {
            eprintln!("[Rust] Failed to get HEAD: {}", e);
            return Err(e.to_string());
        }
    };

    let name = Path::new(path)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("Unknown")
        .to_string();

    let mut opts = StatusOptions::new();
    opts.include_untracked(true);
    let statuses = repo.statuses(Some(&mut opts)).map_err(|e| e.to_string())?;
    let has_changes = !statuses.is_empty();

    let (ahead, behind) = get_ahead_behind(&repo, &current_branch);
    let has_remote = repo.remotes().map(|r| r.len() > 0).unwrap_or(false);

    println!(
        "[Rust] Successfully got repo info for: {} (branch: {})",
        path, current_branch
    );
    Ok(RepoInfo {
        path: path.to_string(),
        name,
        current_branch,
        has_changes,
        ahead,
        behind,
        has_remote,
    })
}

pub fn get_repo_status(repo_path: &str) -> Result<RepoStatus, String> {
    let repo = Repository::open(repo_path).map_err(|e| e.to_string())?;

    // Handle both normal branches and unborn branches
    let current_branch = match repo.head() {
        Ok(head) => head.shorthand().unwrap_or("HEAD").to_string(),
        Err(e) if e.code() == git2::ErrorCode::UnbornBranch => "main".to_string(),
        Err(e) => return Err(e.to_string()),
    };

    let mut opts = StatusOptions::new();
    opts.include_untracked(true);
    opts.include_ignored(false);
    opts.recurse_untracked_dirs(true);

    let statuses = repo.statuses(Some(&mut opts)).map_err(|e| e.to_string())?;

    println!(
        "[Rust] Found {} status entries for {}",
        statuses.len(),
        repo_path
    );

    let mut files = Vec::new();
    for entry in statuses.iter() {
        let status = entry.status();
        let file_path = entry.path().unwrap_or("").to_string();
        
        println!(
            "[Rust] Raw entry: {} with status bits: {:?}",
            file_path, status
        );

        // Skip directories (only include actual files)
        // Check if path exists first to avoid issues with deleted files
        let full_path = Path::new(repo_path).join(&file_path);
        if full_path.exists() && full_path.is_dir() {
            println!("[Rust] Skipping directory: {}", file_path);
            continue;
        }

        println!(
            "[Rust] Processing file: {} with status bits: {:?}",
            file_path, status
        );

        let status_str = match status {
            s if s.contains(Status::INDEX_NEW) => "new",
            s if s.contains(Status::INDEX_MODIFIED) => "modified",
            s if s.contains(Status::INDEX_DELETED) => "deleted",
            s if s.contains(Status::INDEX_RENAMED) => "renamed",
            s if s.contains(Status::WT_MODIFIED) => "modified",
            s if s.contains(Status::WT_DELETED) => "deleted",
            s if s.contains(Status::WT_NEW) => "untracked",
            _ => "unknown",
        };

        let staged = status.intersects(
            Status::INDEX_NEW
                | Status::INDEX_MODIFIED
                | Status::INDEX_DELETED
                | Status::INDEX_RENAMED
                | Status::INDEX_TYPECHANGE,
        );

        files.push(FileStatus {
            path: file_path.clone(),
            status: status_str.to_string(),
            staged,
        });

        println!(
            "[Rust] Added file: {} (status: {}, staged: {})",
            file_path, status_str, staged
        );
    }

    let (ahead, behind) = get_ahead_behind(&repo, &current_branch);
    let has_changes = !files.is_empty() || ahead > 0 || behind > 0;
    let has_remote = repo.remotes().map(|r| r.len() > 0).unwrap_or(false);

    println!("[Rust] Returning {} files for {}", files.len(), repo_path);

    let result = RepoStatus {
        current_branch,
        files,
        ahead,
        behind,
        has_changes,
        has_remote,
    };

    println!(
        "[Rust] RepoStatus created: has_changes={}, files.len()={}",
        result.has_changes,
        result.files.len()
    );
    if let Ok(json) = serde_json::to_string(&result) {
        println!("[Rust] Serialized JSON length: {}", json.len());
    }

    Ok(result)
}

pub fn stage_file(repo_path: &str, file_path: &str) -> Result<(), String> {
    let repo = Repository::open(repo_path).map_err(|e| e.to_string())?;
    let mut index = repo.index().map_err(|e| e.to_string())?;

    index
        .add_path(Path::new(file_path))
        .map_err(|e| e.to_string())?;
    index.write().map_err(|e| e.to_string())?;

    Ok(())
}

pub fn unstage_file(path: &str, file_path: &str) -> Result<(), String> {
    let repo = Repository::open(path).map_err(|e| e.to_string())?;

    // Check if we have any commits (HEAD exists)
    match repo.head() {
        Ok(head) => {
            // Repository has commits, reset to HEAD
            let head_commit = head.peel_to_commit().map_err(|e| e.to_string())?;
            repo.reset_default(Some(&head_commit.as_object()), &[Path::new(file_path)])
                .map_err(|e| e.to_string())?;
        }
        Err(e) if e.code() == git2::ErrorCode::UnbornBranch => {
            // No commits yet, just remove from index
            let mut index = repo.index().map_err(|e| e.to_string())?;
            index
                .remove_path(Path::new(file_path))
                .map_err(|e| e.to_string())?;
            index.write().map_err(|e| e.to_string())?;
        }
        Err(e) => return Err(e.to_string()),
    }

    Ok(())
}

pub fn commit(path: &str, message: &str) -> Result<String, String> {
    let repo = Repository::open(path).map_err(|e| e.to_string())?;
    let mut index = repo.index().map_err(|e| e.to_string())?;
    let tree_id = index.write_tree().map_err(|e| e.to_string())?;
    let tree = repo.find_tree(tree_id).map_err(|e| e.to_string())?;

    let signature = repo.signature().map_err(|e| e.to_string())?;

    // Check if this is the first commit (no HEAD yet)
    let oid = match repo.head() {
        Ok(head) => {
            // Repository has commits, create commit with parent
            let parent_commit = head.peel_to_commit().map_err(|e| e.to_string())?;
            repo.commit(
                Some("HEAD"),
                &signature,
                &signature,
                message,
                &tree,
                &[&parent_commit],
            )
            .map_err(|e| e.to_string())?
        }
        Err(e) if e.code() == git2::ErrorCode::UnbornBranch => {
            // First commit, no parent
            repo.commit(Some("HEAD"), &signature, &signature, message, &tree, &[])
                .map_err(|e| e.to_string())?
        }
        Err(e) => return Err(e.to_string()),
    };

    Ok(oid.to_string())
}

pub fn get_commits(path: &str, limit: usize) -> Result<Vec<CommitInfo>, String> {
    let repo = Repository::open(path).map_err(|e| e.to_string())?;
    let mut revwalk = repo.revwalk().map_err(|e| e.to_string())?;

    // Try to push HEAD, but handle unborn branch (no commits yet)
    match revwalk.push_head() {
        Ok(_) => {}
        Err(e) if e.code() == git2::ErrorCode::UnbornBranch => {
            // No commits yet in this repository
            return Ok(Vec::new());
        }
        Err(e) => return Err(e.to_string()),
    }

    let mut commits = Vec::new();
    for (i, oid) in revwalk.enumerate() {
        if i >= limit {
            break;
        }

        let oid = oid.map_err(|e| e.to_string())?;
        let commit = repo.find_commit(oid).map_err(|e| e.to_string())?;

        commits.push(CommitInfo {
            id: commit.id().to_string(),
            short_id: format!("{:.7}", commit.id()),
            message: commit.message().unwrap_or("").to_string(),
            author: commit.author().name().unwrap_or("Unknown").to_string(),
            timestamp: commit.time().seconds(),
        });
    }

    Ok(commits)
}

pub fn pull(path: &str) -> Result<String, String> {
    let repo = Repository::open(path).map_err(|e| e.to_string())?;

    // Fetch
    let mut remote = repo.find_remote("origin").map_err(|e| e.to_string())?;
    remote
        .fetch(&["HEAD"], None, None)
        .map_err(|e| e.to_string())?;

    // Get the fetch head
    let fetch_head = repo
        .find_reference("FETCH_HEAD")
        .map_err(|e| e.to_string())?;
    let fetch_commit = repo
        .reference_to_annotated_commit(&fetch_head)
        .map_err(|e| e.to_string())?;

    // Merge
    let analysis = repo
        .merge_analysis(&[&fetch_commit])
        .map_err(|e| e.to_string())?;

    if analysis.0.is_fast_forward() {
        let refname = format!(
            "refs/heads/{}",
            repo.head().unwrap().shorthand().unwrap_or("master")
        );
        let mut reference = repo.find_reference(&refname).map_err(|e| e.to_string())?;
        reference
            .set_target(fetch_commit.id(), "Fast-Forward")
            .map_err(|e| e.to_string())?;
        repo.set_head(&refname).map_err(|e| e.to_string())?;
        repo.checkout_head(Some(git2::build::CheckoutBuilder::default().force()))
            .map_err(|e| e.to_string())?;
        Ok("Fast-forward successful".to_string())
    } else if analysis.0.is_up_to_date() {
        Ok("Already up to date".to_string())
    } else {
        Ok("Merge required (not implemented in MVP)".to_string())
    }
}

pub fn push(path: &str) -> Result<String, String> {
    push_with_credentials(path, None, None)
}

pub fn push_with_credentials(
    path: &str,
    username: Option<String>,
    password: Option<String>,
) -> Result<String, String> {
    use git2::{Cred, PushOptions, RemoteCallbacks};

    let repo = Repository::open(path).map_err(|e| e.to_string())?;
    let mut remote = repo.find_remote("origin").map_err(|e| e.to_string())?;

    let head = repo.head().map_err(|e| e.to_string())?;
    let branch_name = head.shorthand().unwrap_or("master");
    let refspec = format!("refs/heads/{}:refs/heads/{}", branch_name, branch_name);

    // Setup callbacks for authentication
    let mut callbacks = RemoteCallbacks::new();

    let username_clone = username.clone();
    let password_clone = password.clone();

    callbacks.credentials(move |_url, username_from_url, _allowed_types| {
        if let (Some(user), Some(pass)) = (&username_clone, &password_clone) {
            // Use provided credentials (HTTPS with PAT)
            Cred::userpass_plaintext(user, pass)
        } else if let Some(user) = username_from_url {
            // Try SSH agent
            Cred::ssh_key_from_agent(user)
        } else {
            // Default SSH agent
            Cred::ssh_key_from_agent("git")
        }
    });

    let mut push_options = PushOptions::new();
    push_options.remote_callbacks(callbacks);

    match remote.push(&[&refspec], Some(&mut push_options)) {
        Ok(_) => {
            // Set upstream tracking
            if let Ok(mut branch) = repo.find_branch(&branch_name, BranchType::Local) {
                let _ = branch.set_upstream(Some(&format!("origin/{}", branch_name)));
            }
            Ok("Push successful".to_string())
        }
        Err(e) => {
            let error_msg = e.to_string();
            if error_msg.contains("401")
                || error_msg.contains("403")
                || error_msg.contains("authentication")
            {
                Err("Authentication failed. Please check your GitHub username and Personal Access Token have the correct permissions (repo scope required).".to_string())
            } else {
                Err(error_msg)
            }
        }
    }
}

pub fn get_branches(path: &str) -> Result<Vec<BranchInfo>, String> {
    let repo = Repository::open(path).map_err(|e| e.to_string())?;

    // Check if repo has any commits (handle unborn branch)
    let current_branch = match repo.head() {
        Ok(head) => head.shorthand().unwrap_or("").to_string(),
        Err(e) if e.code() == git2::ErrorCode::UnbornBranch => {
            // No commits yet, return default main branch
            return Ok(vec![BranchInfo {
                name: "main".to_string(),
                is_current: true,
                is_remote: false,
            }]);
        }
        Err(e) => return Err(e.to_string()),
    };

    let mut branches = Vec::new();

    // Local branches
    let local_branches = repo
        .branches(Some(BranchType::Local))
        .map_err(|e| e.to_string())?;
    for branch in local_branches {
        let (branch, _) = branch.map_err(|e| e.to_string())?;
        if let Some(name) = branch.name().map_err(|e| e.to_string())? {
            branches.push(BranchInfo {
                name: name.to_string(),
                is_current: name == current_branch,
                is_remote: false,
            });
        }
    }

    // Remote branches
    let remote_branches = repo
        .branches(Some(BranchType::Remote))
        .map_err(|e| e.to_string())?;
    for branch in remote_branches {
        let (branch, _) = branch.map_err(|e| e.to_string())?;
        if let Some(name) = branch.name().map_err(|e| e.to_string())? {
            branches.push(BranchInfo {
                name: name.to_string(),
                is_current: false,
                is_remote: true,
            });
        }
    }

    Ok(branches)
}

pub fn checkout_branch(path: &str, branch_name: &str) -> Result<(), String> {
    let repo = Repository::open(path).map_err(|e| e.to_string())?;
    let obj = repo
        .revparse_single(&format!("refs/heads/{}", branch_name))
        .map_err(|e| e.to_string())?;

    repo.checkout_tree(&obj, None).map_err(|e| e.to_string())?;
    repo.set_head(&format!("refs/heads/{}", branch_name))
        .map_err(|e| e.to_string())?;

    Ok(())
}

fn get_ahead_behind(repo: &Repository, branch_name: &str) -> (usize, usize) {
    let local_branch = match repo.find_branch(branch_name, BranchType::Local) {
        Ok(b) => b,
        Err(_) => return (0, 0),
    };

    let upstream = match local_branch.upstream() {
        Ok(u) => u,
        Err(_) => return (0, 0),
    };

    let local_oid = match local_branch.get().target() {
        Some(oid) => oid,
        None => return (0, 0),
    };

    let upstream_oid = match upstream.get().target() {
        Some(oid) => oid,
        None => return (0, 0),
    };

    match repo.graph_ahead_behind(local_oid, upstream_oid) {
        Ok((ahead, behind)) => (ahead, behind),
        Err(_) => (0, 0),
    }
}

pub fn clone_repository(url: &str, path: &str) -> Result<String, String> {
    Repository::clone(url, path).map_err(|e| e.to_string())?;
    Ok(format!("Successfully cloned repository to {}", path))
}

pub fn init_repository(path: &str) -> Result<String, String> {
    println!("[Rust Init] Initializing repository at: {}", path);
    match Repository::init(path) {
        Ok(_) => {
            println!("[Rust Init] Successfully initialized at: {}", path);
            Ok(format!("Successfully initialized repository at {}", path))
        }
        Err(e) => {
            eprintln!("[Rust Init] Failed to initialize: {}", e);
            Err(e.to_string())
        }
    }
}
pub fn add_remote(path: &str, name: &str, url: &str) -> Result<(), String> {
    let repo = Repository::open(path).map_err(|e| e.to_string())?;
    repo.remote(name, url).map_err(|e| e.to_string())?;
    Ok(())
}
