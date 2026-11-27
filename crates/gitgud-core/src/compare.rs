use serde::{Deserialize, Serialize};

use crate::cli::GitCli;
use crate::diff::{FileChange, FileDiff, blob_at, file_change, file_diff_in_repo, find_commit};
use crate::repo::{Change, Error, Repo, read_err};
use crate::unity_yaml::{UnityYamlDiff, unity_diff_of};
use crate::worktree::Versions;

/// Commits listed per side; the count says how many there are in all.
pub const LISTED_COMMITS: usize = 500;

/// One side of a comparison (R12-B).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum Point {
    /// A branch, tag, stash (`stash@{0}`), commit id or anything `rev-parse` takes.
    Rev { rev: String },
    /// The files on disk, uncommitted changes included.
    Worktree,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Mode {
    /// What B brings: B against where A and B split (`git diff A...B`).
    Brings,
    /// The two snapshots against each other (`git diff A B`).
    Snapshot,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CompareCommit {
    pub id: String,
    pub short_id: String,
    pub subject: String,
    pub author: String,
    pub time: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CommitList {
    /// All commits only on this side (merges left out).
    pub count: u32,
    /// The newest [`LISTED_COMMITS`] of them.
    pub commits: Vec<CompareCommit>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Comparison {
    /// A's commit (HEAD's for the working copy).
    pub a: String,
    pub b: String,
    /// Where A and B split; `None` when they share no history.
    pub base: Option<String>,
    /// The mode used: `Snapshot` when `Brings` can't apply (no shared history, or a working
    /// copy on the A side).
    pub mode: Mode,
    /// The commit the "before" versions come from: the base (brings) or A (snapshot).
    pub from: String,
    pub only_a: CommitList,
    pub only_b: CommitList,
    /// Sorted by path, no rename detection.
    pub files: Vec<FileChange>,
}

/// Compares `a` with `b` in `mode`.
///
/// # Errors
/// [`Error::Refused`] for a side that isn't a commit here, else git / read failures.
pub fn compare(
    git: &GitCli,
    repo: &Repo,
    a: &Point,
    b: &Point,
    mode: Mode,
) -> Result<Comparison, Error> {
    let a_id = commit_of(git, repo, a)?;
    let b_id = commit_of(git, repo, b)?;
    let base = merge_base(git, repo, &a_id, &b_id)?;
    let mode = match (mode, &base, a) {
        (Mode::Brings, Some(_), Point::Rev { .. }) => Mode::Brings,
        _ => Mode::Snapshot,
    };
    let from = match (mode, &base) {
        (Mode::Brings, Some(base)) => base.clone(),
        _ => a_id.clone(),
    };
    let files = match (a, b) {
        (Point::Worktree, Point::Worktree) => Vec::new(),
        (_, Point::Worktree) => worktree_changes(git, repo, &from)?,
        (Point::Worktree, _) => worktree_changes(git, repo, &b_id)?
            .into_iter()
            .map(|f| FileChange {
                change: reversed(f.change),
                ..f
            })
            .collect(),
        _ => tree_changes(repo, &from, &b_id)?,
    };
    Ok(Comparison {
        only_a: commits(git, repo, &b_id, &a_id)?,
        only_b: commits(git, repo, &a_id, &b_id)?,
        a: a_id,
        b: b_id,
        base,
        mode,
        from,
        files,
    })
}

/// The text diff of `path` between `from` (a commit) and `to`, as [`Comparison`] describes them.
///
/// # Errors
/// [`Error::Read`] when neither side has `path`.
pub fn file_diff(
    repo: &Repo,
    from: &Point,
    to: &Point,
    path: &str,
    ignore_whitespace: bool,
) -> Result<FileDiff, Error> {
    let v = versions(repo, from, to, path)?;
    Ok(file_diff_in_repo(
        Some(repo),
        path,
        v.old.as_deref(),
        v.new.as_deref(),
        ignore_whitespace,
    ))
}

/// The structural Unity YAML diff of `path` between `from` and `to`.
///
/// # Errors
/// [`Error::Refused`] for a file that isn't text Unity YAML.
pub fn unity_diff(
    repo: &Repo,
    from: &Point,
    to: &Point,
    path: &str,
) -> Result<UnityYamlDiff, Error> {
    unity_diff_of(repo, path, versions(repo, from, to, path)?)
}

/// Makes the working copy's `path` what it is at `rev`, staged: B's version, or gone when
/// `rev` doesn't have it.
///
/// # Errors
/// Git's error.
pub fn take_version(git: &GitCli, repo: &Repo, rev: &str, path: &str) -> Result<(), Error> {
    let id = commit_of(
        git,
        repo,
        &Point::Rev {
            rev: rev.to_owned(),
        },
    )?;
    let gix = repo.gix();
    let commit = find_commit(&gix, &id)?;
    let input = format!("{path}\0");
    if blob_at(&gix, &commit, path)?.is_some() {
        git.run(
            repo.root(),
            &[
                "--literal-pathspecs",
                "checkout",
                &id,
                "--pathspec-from-file=-",
                "--pathspec-file-nul",
            ],
            Some(input.as_bytes()),
        )?;
    } else {
        git.run(
            repo.root(),
            &[
                "--literal-pathspecs",
                "rm",
                "-q",
                "-f",
                "--ignore-unmatch",
                "--pathspec-from-file=-",
                "--pathspec-file-nul",
            ],
            Some(input.as_bytes()),
        )?;
    }
    Ok(())
}

fn versions(repo: &Repo, from: &Point, to: &Point, path: &str) -> Result<Versions, Error> {
    let gix = repo.gix();
    let read = |p: &Point| -> Result<Option<Vec<u8>>, Error> {
        match p {
            Point::Rev { rev } => {
                let id = gix.rev_parse_single(rev.as_str()).map_err(read_err)?;
                let commit = id
                    .object()
                    .map_err(read_err)?
                    .peel_to_commit()
                    .map_err(read_err)?;
                blob_at(&gix, &commit, path)
            }
            Point::Worktree => repo.worktree_as_git(&gix, path),
        }
    };
    let v = Versions {
        old: read(from)?,
        new: read(to)?,
    };
    if v.old.is_none() && v.new.is_none() {
        return Err(Error::Read(format!("{path} is on neither side")));
    }
    Ok(v)
}

/// The commit a side stands for; the working copy stands on HEAD.
fn commit_of(git: &GitCli, repo: &Repo, p: &Point) -> Result<String, Error> {
    let rev = match p {
        Point::Rev { rev } => rev.as_str(),
        Point::Worktree => "HEAD",
    };
    repo.resolve_commit(git, rev)
}

fn merge_base(git: &GitCli, repo: &Repo, a: &str, b: &str) -> Result<Option<String>, Error> {
    let (code, out, err) = git.run_status(repo.root(), &["merge-base", a, b], None)?;
    match code {
        0 => Ok(Some(out.trim().to_owned())),
        1 => Ok(None),
        _ => Err(Error::Read(format!("git merge-base failed: {err}"))),
    }
}

/// Commits on `to` that aren't on `from`, newest first.
fn commits(git: &GitCli, repo: &Repo, from: &str, to: &str) -> Result<CommitList, Error> {
    if from == to {
        return Ok(CommitList {
            count: 0,
            commits: Vec::new(),
        });
    }
    let range = format!("{from}..{to}");
    let count: u32 = git
        .run(
            repo.root(),
            &["rev-list", "--count", "--no-merges", &range],
            None,
        )?
        .trim()
        .parse()
        .map_err(|_| Error::Read("unexpected rev-list --count output".into()))?;
    let limit = format!("-n{LISTED_COMMITS}");
    let out = git.run(
        repo.root(),
        &[
            "log",
            "--no-merges",
            &limit,
            "--format=%H%x1f%h%x1f%an%x1f%at%x1f%s%x1e",
            &range,
        ],
        None,
    )?;
    let commits = out
        .split('\x1e')
        .map(str::trim)
        .filter(|r| !r.is_empty())
        .map(|r| {
            let f: Vec<&str> = r.splitn(5, '\x1f').collect();
            let [id, short, author, time, subject] = f[..] else {
                return Err(Error::Read(format!("unexpected git log output: {r:?}")));
            };
            Ok(CompareCommit {
                id: id.to_owned(),
                short_id: short.to_owned(),
                subject: subject.to_owned(),
                author: author.to_owned(),
                time: time
                    .parse()
                    .map_err(|_| Error::Read(format!("bad commit time {time:?}")))?,
            })
        })
        .collect::<Result<_, _>>()?;
    Ok(CommitList { count, commits })
}

fn tree_changes(repo: &Repo, from: &str, to: &str) -> Result<Vec<FileChange>, Error> {
    let gix = repo.gix();
    let old = find_commit(&gix, from)?.tree().map_err(read_err)?;
    let new = find_commit(&gix, to)?.tree().map_err(read_err)?;
    let changes = gix
        .diff_tree_to_tree(
            &old,
            &new,
            gix::diff::Options::default().with_rewrites(None),
        )
        .map_err(read_err)?;
    let mut files: Vec<FileChange> = changes.iter().filter_map(file_change).collect();
    files.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(files)
}

/// The work tree (tracked changes and untracked files) against commit `from`.
fn worktree_changes(git: &GitCli, repo: &Repo, from: &str) -> Result<Vec<FileChange>, Error> {
    let out = git.run(
        repo.root(),
        &["diff", "--no-renames", "--name-status", "-z", from, "--"],
        None,
    )?;
    let mut parts = out.split('\0').filter(|p| !p.is_empty());
    let mut files = Vec::new();
    while let (Some(status), Some(path)) = (parts.next(), parts.next()) {
        let change = match status.chars().next() {
            Some('A') => Change::Added,
            Some('D') => Change::Deleted,
            Some('T') => Change::TypeChange,
            Some('M') => Change::Modified,
            _ => {
                return Err(Error::Read(format!(
                    "unexpected git diff status {status:?}"
                )));
            }
        };
        files.push(FileChange {
            path: path.to_owned(),
            change,
        });
    }
    for e in repo.status()?.entries {
        if e.kind == crate::repo::EntryKind::Untracked {
            files.push(FileChange {
                path: e.path,
                change: Change::Added,
            });
        }
    }
    files.sort_by(|a, b| a.path.cmp(&b.path));
    files.dedup_by(|a, b| a.path == b.path);
    Ok(files)
}

fn reversed(c: Change) -> Change {
    match c {
        Change::Added => Change::Deleted,
        Change::Deleted => Change::Added,
        other => other,
    }
}
