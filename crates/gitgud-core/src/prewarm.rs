use rayon::prelude::*;

use crate::cli::GitCli;
use crate::repo::{Error, Repo};

/// Staging at least this many paths writes their blobs ahead, in one pack. Below it `git add`'s
/// own loose objects are quick enough, and `fast-import` would explode a small pack into loose
/// objects anyway (`fastimport.unpackLimit`, 100).
pub(crate) const PREWARM_MIN: usize = 100;
/// Larger files are left to `git add`; they are few, and held in memory here.
const PREWARM_MAX_FILE: u64 = 32 << 20;

/// Writes the blobs `git add` is about to write for `paths` into one pack with
/// `git fast-import`, so `git add` finds them present instead of creating one loose object
/// file each (3+ ms apiece on Windows: 15 s for 4,500 new files). The contents go through
/// git's to-git conversion (gix's filter pipeline), and `git add` still hashes every file
/// itself: a blob that came out differently is written by `git add` as usual. LFS files are
/// left out (their clean filter would run twice). Returns how many blobs were written.
///
/// # Errors
/// Git's error from `check-attr` or `fast-import`, or a read failure.
pub(crate) fn prewarm_blobs(git: &GitCli, repo: &Repo, paths: &[String]) -> Result<usize, Error> {
    let lfs = lfs_paths(git, repo, paths)?;
    let files: Vec<&String> = paths
        .iter()
        .filter(|p| !lfs.contains(p.as_str()))
        .filter(|p| {
            std::fs::symlink_metadata(repo.root().join(p))
                .is_ok_and(|m| m.is_file() && m.len() <= PREWARM_MAX_FILE)
        })
        .collect();
    if files.len() < PREWARM_MIN {
        return Ok(0);
    }
    let blobs: Vec<Vec<u8>> = files
        .par_iter()
        .map_init(|| repo.gix(), |gix, p| repo.worktree_as_git(gix, p))
        .filter_map(|r| match r {
            Ok(Some(bytes)) => Some(Ok(bytes)),
            Ok(None) => None,
            Err(e) => Some(Err(e)),
        })
        .collect::<Result<_, _>>()?;
    let mut stream = Vec::with_capacity(blobs.iter().map(|b| b.len() + 32).sum());
    for blob in &blobs {
        stream.extend_from_slice(format!("blob\ndata {}\n", blob.len()).as_bytes());
        stream.extend_from_slice(blob);
        stream.push(b'\n');
    }
    git.run(repo.root(), &["fast-import", "--quiet"], Some(&stream))?;
    Ok(blobs.len())
}

/// The paths among `paths` whose `filter` attribute is `lfs`.
fn lfs_paths(
    git: &GitCli,
    repo: &Repo,
    paths: &[String],
) -> Result<std::collections::HashSet<String>, Error> {
    let mut input = Vec::new();
    for p in paths {
        input.extend_from_slice(p.as_bytes());
        input.push(0);
    }
    let out = git.run(
        repo.root(),
        &["check-attr", "-z", "--stdin", "filter"],
        Some(&input),
    )?;
    let fields: Vec<&str> = out.split('\0').collect();
    Ok(fields
        .as_chunks::<3>()
        .0
        .iter()
        .filter(|[_, _, value]| *value == "lfs")
        .map(|[path, _, _]| (*path).to_owned())
        .collect())
}
