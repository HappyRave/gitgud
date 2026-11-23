use std::collections::BTreeSet;
use std::path::PathBuf;

use crate::cli::GitCli;
use crate::repo::{Error, Repo, read_err};

/// The private ref that keeps every snapshot's content reachable, so `git gc`
/// never prunes something an undo still needs.
pub const SNAPSHOT_REF: &str = "refs/gitgud/snapshots";

/// Records the whole work tree (tracked and untracked files, not ignored ones)
/// as a git tree, without touching the real index (spec §3.3, spike 4.8).
///
/// A persistent snapshot index keeps stat data between snapshots, so files that
/// were only touched (a Unity reimport) aren't hashed again.
pub struct Snapshots {
    index: PathBuf,
}

impl Snapshots {
    pub fn for_repo(repo: &Repo) -> Self {
        Self {
            index: repo.gix().git_dir().join("gitgud").join("snapshot.index"),
        }
    }

    /// Snapshots the work tree and returns the tree id.
    ///
    /// # Errors
    /// [`Error::Command`] when a git step fails.
    pub fn take(&self, repo: &Repo, git: &GitCli) -> Result<String, Error> {
        if let Some(dir) = self.index.parent() {
            std::fs::create_dir_all(dir).map_err(crate::repo::read_err)?;
        }
        let env = [("GIT_INDEX_FILE", self.index.as_path())];
        let root = repo.root();
        git.run_env(root, &["add", "-A", "--", "."], None, &env)?;
        let tree = git
            .run_env(root, &["write-tree"], None, &env)?
            .trim()
            .to_owned();

        let previous = repo
            .gix()
            .try_find_reference(SNAPSHOT_REF)
            .map_err(crate::repo::read_err)?
            .and_then(|r| r.target().try_id().map(|id| id.to_hex().to_string()));
        // A fixed identity: snapshots are internal and must work without user.name.
        let mut args = vec![
            "-c",
            "user.name=gitgud",
            "-c",
            "user.email=snapshots@gitgud.invalid",
            "commit-tree",
            tree.as_str(),
            "-m",
            "gitgud snapshot",
        ];
        if let Some(p) = &previous {
            args.extend(["-p", p.as_str()]);
        }
        let commit = git.run(root, &args, None)?.trim().to_owned();
        let mut update = vec!["update-ref", SNAPSHOT_REF, commit.as_str()];
        if let Some(p) = &previous {
            update.push(p.as_str());
        }
        git.run(root, &update, None)?;
        Ok(tree)
    }

    /// Rewrites [`SNAPSHOT_REF`] so only trees in `kept_trees` remain reachable,
    /// allowing `git gc` to collect expired snapshot trees and commits. Returns `true`
    /// when [`SNAPSHOT_REF`] was updated or deleted.
    ///
    /// # Errors
    /// [`Error::Command`] or [`Error::Read`] when updating git objects/refs fails.
    pub fn sync_kept(repo: &Repo, git: &GitCli, kept_trees: &[String]) -> Result<bool, Error> {
        let gix_repo = repo.gix();
        let mut distinct = BTreeSet::new();
        for hex in kept_trees {
            let Ok(oid) = gix::ObjectId::from_hex(hex.as_bytes()) else {
                continue;
            };
            if gix_repo.find_tree(oid).is_ok() {
                distinct.insert(oid.to_string());
            }
        }

        let log_path = gix_repo.common_dir().join("logs").join(SNAPSHOT_REF);
        let existing = gix_repo
            .try_find_reference(SNAPSHOT_REF)
            .map_err(read_err)?
            .and_then(|r| r.target().try_id().map(ToOwned::to_owned));

        if distinct.is_empty() {
            let Some(_) = existing else {
                let _ = std::fs::remove_file(&log_path);
                return Ok(false);
            };
            git.run(repo.root(), &["update-ref", "-d", SNAPSHOT_REF], None)?;
            let _ = std::fs::remove_file(&log_path);
            return Ok(true);
        }

        let mut tree_entries = String::new();
        for oid in &distinct {
            tree_entries.push_str(&format!("040000 tree {oid}\t{oid}\n"));
        }
        let root_tree = git
            .run(repo.root(), &["mktree"], Some(tree_entries.as_bytes()))?
            .trim()
            .to_owned();

        if let Some(tip) = existing
            && let Ok(commit) = gix_repo.find_commit(tip)
            && commit.parent_ids().next().is_none()
            && commit.tree_id().ok().map(|id| id.to_string()).as_deref() == Some(root_tree.as_str())
        {
            let _ = std::fs::remove_file(&log_path);
            return Ok(false);
        }

        let commit = git
            .run(
                repo.root(),
                &[
                    "-c",
                    "user.name=gitgud",
                    "-c",
                    "user.email=snapshots@gitgud.invalid",
                    "commit-tree",
                    root_tree.as_str(),
                    "-m",
                    "gitgud snapshots",
                ],
                None,
            )?
            .trim()
            .to_owned();
        git.run(
            repo.root(),
            &["update-ref", SNAPSHOT_REF, commit.as_str()],
            None,
        )?;
        let _ = std::fs::remove_file(&log_path);
        Ok(true)
    }
}
