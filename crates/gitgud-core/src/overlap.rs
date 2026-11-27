use std::collections::{BTreeMap, BTreeSet, HashMap};

use serde::Serialize;

use crate::cli::GitCli;
use crate::diff::is_binary;
use crate::repo::{Error, Repo, read_err};

/// Commits on the main branch older than this aren't news: a feature branch cut long ago
/// would otherwise flag everything trunk changed since.
const TRUNK_WINDOW: &str = "--since=7.days";

/// How a file both sides changed will merge.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Risk {
    /// LFS, binary, or deleted on one side: git can't merge it line by line.
    WontMerge,
    /// Text where both sides changed the same lines.
    MayConflict,
    /// Text changed in different places.
    Merges,
}

/// A file you changed that also changed on a remote branch since your merge base (R11-B).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OverlapFile {
    /// The asset's path; its `.meta` is folded in.
    pub path: String,
    pub risk: Risk,
    /// The remote branch it changed on, e.g. `origin/PartyGame`.
    pub on: String,
    /// `on` is the main branch, not your upstream: pulling won't bring it, merging it will.
    pub trunk: bool,
    /// Authors of those commits, newest first, each once.
    pub authors: Vec<String>,
    pub commits: u32,
    /// Unix seconds of the newest of those commits.
    pub newest: i64,
    pub newest_commit: String,
    /// The working copy has changes to it (else only your unpushed commits do).
    pub dirty: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Overlap {
    /// The remote branches compared with, upstream first.
    pub against: Vec<String>,
    /// Won't-merge first, then newest first.
    pub files: Vec<OverlapFile>,
}

struct Theirs {
    commits: u32,
    authors: Vec<String>,
    newest: i64,
    newest_commit: String,
}

/// Files changed both in your work (working copy and commits since the merge base) and on
/// your upstream, or on the main branch's last week when that's another branch. `None`
/// when there's nothing to compare with (no upstream, no `origin/HEAD` / main / master).
///
/// # Errors
/// Git or read failures.
pub fn overlap(git: &GitCli, repo: &Repo) -> Result<Option<Overlap>, Error> {
    let targets = targets(git, repo)?;
    if targets.is_empty() {
        return Ok(None);
    }
    let status = repo.status()?;
    let dirty: BTreeSet<String> = status.entries.into_iter().map(|e| e.path).collect();
    let mut seen = BTreeSet::new();
    let mut files = Vec::new();
    for (target, trunk) in &targets {
        let Some(base) = merge_base(git, repo, target)? else {
            continue;
        };
        let theirs = their_changes(git, repo, &base, target, *trunk)?;
        if theirs.is_empty() {
            continue;
        }
        let mut mine: BTreeSet<String> = lines(&git.run(
            repo.root(),
            &["diff", "--no-renames", "--name-only", "-z", &base, "HEAD"],
            None,
        )?);
        mine.extend(dirty.iter().cloned());
        let both: Vec<&String> = mine.iter().filter(|p| theirs.contains_key(*p)).collect();
        if both.is_empty() {
            continue;
        }
        let attrs = merge_attrs(git, repo, &both)?;
        let mut grouped: BTreeMap<String, OverlapFile> = BTreeMap::new();
        for path in both {
            let asset = asset_of(path).to_owned();
            if seen.contains(&asset) {
                continue;
            }
            let risk = if attrs.contains(path) {
                Risk::WontMerge
            } else {
                text_risk(git, repo, &base, target, path)?
            };
            let t = &theirs[path];
            let is_dirty = dirty.contains(path);
            let entry = grouped.entry(asset.clone()).or_insert_with(|| OverlapFile {
                path: asset,
                risk,
                on: target.clone(),
                trunk: *trunk,
                authors: Vec::new(),
                commits: 0,
                newest: t.newest,
                newest_commit: t.newest_commit.clone(),
                dirty: false,
            });
            entry.risk = entry.risk.min(risk);
            entry.dirty |= is_dirty;
            entry.commits = entry.commits.max(t.commits);
            if t.newest > entry.newest {
                entry.newest = t.newest;
                entry.newest_commit.clone_from(&t.newest_commit);
            }
            for a in &t.authors {
                if !entry.authors.contains(a) {
                    entry.authors.push(a.clone());
                }
            }
        }
        seen.extend(grouped.keys().cloned());
        files.extend(grouped.into_values());
    }
    files.sort_by(|a, b| a.risk.cmp(&b.risk).then(b.newest.cmp(&a.newest)));
    Ok(Some(Overlap {
        against: targets.into_iter().map(|(t, _)| t).collect(),
        files,
    }))
}

/// The repo-local config key that turns the main-branch check off (default on).
const MAIN_CHECK_KEY: &str = "gitgud.overlapMain";

/// Whether the radar also checks the main branch's last week in this repo.
///
/// # Errors
/// Git failures, or a value git can't read as a bool.
pub fn main_check(git: &GitCli, repo: &Repo) -> Result<bool, Error> {
    let out = git.run(
        repo.root(),
        &[
            "config",
            "--type=bool",
            "--default=true",
            "--get",
            MAIN_CHECK_KEY,
        ],
        None,
    )?;
    Ok(out.trim() != "false")
}

/// Turns the main-branch check on (removes the key) or off, in the repo's local config.
///
/// # Errors
/// Git failures.
pub fn set_main_check(git: &GitCli, repo: &Repo, on: bool) -> Result<(), Error> {
    if !on {
        git.run(
            repo.root(),
            &["config", "--local", MAIN_CHECK_KEY, "false"],
            None,
        )?;
        return Ok(());
    }
    let args = ["config", "--local", "--unset-all", MAIN_CHECK_KEY];
    let (code, out, err) = git.run_status(repo.root(), &args, None)?;
    // 5: the key wasn't set.
    if code != 0 && code != 5 {
        return Err(crate::cli::command_error(
            &args,
            crate::cli::failure_message(&out, &err),
        ));
    }
    Ok(())
}

/// The upstream, then the main branch (flagged `true`) when it's another branch and the
/// repo hasn't turned that check off.
fn targets(git: &GitCli, repo: &Repo) -> Result<Vec<(String, bool)>, Error> {
    let run = |args: &[&str]| {
        git.run(repo.root(), args, None)
            .ok()
            .map(|s| s.trim().to_owned())
            .filter(|s| !s.is_empty())
    };
    let mut out = Vec::new();
    if let Some(up) = run(&[
        "rev-parse",
        "--abbrev-ref",
        "--symbolic-full-name",
        "@{upstream}",
    ]) {
        out.push((up, false));
    }
    if !main_check(git, repo)? {
        return Ok(out);
    }
    let trunk = run(&["symbolic-ref", "-q", "--short", "refs/remotes/origin/HEAD"]).or_else(|| {
        ["origin/main", "origin/master"]
            .into_iter()
            .find(|b| run(&["rev-parse", "--verify", "-q", b]).is_some())
            .map(str::to_owned)
    });
    if let Some(trunk) = trunk
        && !out.iter().any(|(t, _)| *t == trunk)
    {
        out.push((trunk, true));
    }
    Ok(out)
}

fn merge_base(git: &GitCli, repo: &Repo, target: &str) -> Result<Option<String>, Error> {
    let (code, out, _) = git.run_status(repo.root(), &["merge-base", "HEAD", target], None)?;
    Ok((code == 0).then(|| out.trim().to_owned()))
}

/// Paths changed on `target` since `base`, with who and when.
fn their_changes(
    git: &GitCli,
    repo: &Repo,
    base: &str,
    target: &str,
    trunk: bool,
) -> Result<HashMap<String, Theirs>, Error> {
    let range = format!("{base}..{target}");
    let mut args = vec![
        "log",
        "--no-renames",
        "--name-only",
        "-z",
        "--format=%x1e%H%x1f%an%x1f%ct",
    ];
    if trunk {
        args.push(TRUNK_WINDOW);
    }
    args.extend([range.as_str(), "--"]);
    let out = git.run(repo.root(), &args, None)?;
    let mut map: HashMap<String, Theirs> = HashMap::new();
    for record in out.split('\x1e').filter(|r| !r.is_empty()) {
        let mut parts = record.splitn(3, '\x1f');
        let (Some(id), Some(author), Some(rest)) = (parts.next(), parts.next(), parts.next())
        else {
            return Err(Error::Read(format!(
                "unexpected git log output: {record:?}"
            )));
        };
        // With -z the format line ends in NUL and the names follow, NUL-separated.
        let mut fields = rest
            .split(['\0', '\n'])
            .map(str::trim)
            .filter(|p| !p.is_empty());
        let time = fields.next().unwrap_or_default();
        let time: i64 = time
            .parse()
            .map_err(|_| Error::Read(format!("unexpected commit time {time:?}")))?;
        for path in fields {
            let t = map.entry(path.to_owned()).or_insert_with(|| Theirs {
                commits: 0,
                authors: Vec::new(),
                newest: time,
                newest_commit: id.to_owned(),
            });
            t.commits += 1;
            if !t.authors.iter().any(|a| a == author) {
                t.authors.push(author.to_owned());
            }
        }
    }
    Ok(map)
}

/// The paths git won't merge as text: LFS, or `merge` unset / `binary`.
fn merge_attrs(git: &GitCli, repo: &Repo, paths: &[&String]) -> Result<BTreeSet<String>, Error> {
    let mut input = Vec::new();
    for p in paths {
        input.extend_from_slice(p.as_bytes());
        input.push(0);
    }
    let out = git.run(
        repo.root(),
        &["check-attr", "-z", "--stdin", "filter", "merge"],
        Some(&input),
    )?;
    let fields: Vec<&str> = out.split('\0').collect();
    Ok(fields
        .as_chunks::<3>()
        .0
        .iter()
        .filter(|[_, attr, value]| {
            (*attr == "filter" && *value == "lfs")
                || (*attr == "merge" && (*value == "unset" || *value == "binary"))
        })
        .map(|[path, _, _]| (*path).to_owned())
        .collect())
}

/// Merges base, `target`'s version and the work tree's (all in git form) with `merge-file`,
/// writing nothing to the work tree.
fn text_risk(
    git: &GitCli,
    repo: &Repo,
    base: &str,
    target: &str,
    path: &str,
) -> Result<Risk, Error> {
    let gix = repo.gix();
    let blob = |rev: &str| -> Result<Option<Vec<u8>>, Error> {
        let id = gix.rev_parse_single(rev).map_err(read_err)?;
        let commit = id
            .object()
            .map_err(read_err)?
            .peel_to_commit()
            .map_err(read_err)?;
        crate::diff::blob_at(&gix, &commit, path)
    };
    let (theirs, mine) = match (blob(target)?, repo.worktree_as_git(&gix, path)?) {
        (Some(t), Some(m)) => (t, m),
        (None, None) => return Ok(Risk::Merges),
        _ => return Ok(Risk::WontMerge),
    };
    let old = blob(base)?.unwrap_or_default();
    if is_binary(&old) || is_binary(&theirs) || is_binary(&mine) {
        return Ok(Risk::WontMerge);
    }
    let dir = crate::repo::Scratch::new(repo, "overlap")?;
    let (m, b, t) = (
        dir.write("mine", &mine)?,
        dir.write("base", &old)?,
        dir.write("theirs", &theirs)?,
    );
    let (code, _, err) = git.run_status(
        repo.root(),
        &["merge-file", "-p", "--quiet", &m, &b, &t],
        None,
    )?;
    if !(0..=127).contains(&code) {
        return Err(Error::Read(format!(
            "git merge-file failed on {path}: {err}"
        )));
    }
    Ok(if code == 0 {
        Risk::Merges
    } else {
        Risk::MayConflict
    })
}

fn asset_of(path: &str) -> &str {
    path.strip_suffix(".meta").unwrap_or(path)
}

fn lines(out: &str) -> BTreeSet<String> {
    out.split(['\0', '\n'])
        .map(str::trim)
        .filter(|p| !p.is_empty())
        .map(str::to_owned)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn meta_folds_into_its_asset() {
        assert_eq!(asset_of("Assets/Lobby.unity.meta"), "Assets/Lobby.unity");
        assert_eq!(asset_of("Assets/Lobby.unity"), "Assets/Lobby.unity");
        assert_eq!(asset_of("Assets/Folder.meta"), "Assets/Folder");
    }

    #[test]
    fn risks_sort_worst_first() {
        let mut r = vec![Risk::Merges, Risk::WontMerge, Risk::MayConflict];
        r.sort();
        assert_eq!(r, [Risk::WontMerge, Risk::MayConflict, Risk::Merges]);
    }

    #[test]
    fn lines_split_nul_and_newline_output() {
        let got = lines("a.cs\0b dir/c.unity\0\n");
        assert_eq!(
            got.into_iter().collect::<Vec<_>>(),
            ["a.cs", "b dir/c.unity"]
        );
    }
}
