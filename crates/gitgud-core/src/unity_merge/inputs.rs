use serde::Serialize;

use super::{MergedFile, PropertyMerge, RowPick, analyze, merge};
use crate::diff::is_binary;
use crate::lfs::{lfs_is_hydrated, lfs_object_path, parse_lfs_pointer};
use crate::repo::{Error, Repo, read_err};
use crate::unity_yaml::{is_unity_yaml_path, repo_guid_index};

/// An LFS object a property merge needs that isn't downloaded yet.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MissingObject {
    /// `base`, `yours` or `theirs`.
    pub side: String,
    pub oid: String,
    pub size: u64,
}

/// Whether a conflicted file can be merged by property, and what that would do (R9's offer).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum MergeOffer {
    Ready {
        yours: usize,
        theirs: usize,
        clashes: usize,
        objects: usize,
    },
    NeedsFetch {
        missing: Vec<MissingObject>,
    },
    Unavailable {
        reason: String,
    },
}

struct Versions {
    base: String,
    ours: String,
    theirs: String,
    /// Stored in LFS: the texts are the work-tree form.
    lfs: bool,
}

enum Read {
    Ready(Versions),
    NeedsFetch(Vec<MissingObject>),
    Unavailable(String),
}

/// The three versions of conflicted `path` as text, LFS objects read from the local store.
fn read(repo: &Repo, path: &str) -> Result<Read, Error> {
    if !is_unity_yaml_path(path) {
        return Ok(Read::Unavailable(format!("{path} isn't a Unity YAML file")));
    }
    let [base, ours, theirs] = repo.stages(path)?;
    let (Some(ours), Some(theirs)) = (ours, theirs) else {
        return Ok(Read::Unavailable("one side deleted the file".into()));
    };
    let mut missing = Vec::new();
    let mut texts = Vec::new();
    let mut lfs = false;
    for (side, bytes) in [
        ("base", base.unwrap_or_default()),
        ("yours", ours),
        ("theirs", theirs),
    ] {
        let bytes = match parse_lfs_pointer(&bytes) {
            Some((oid, size)) if !lfs_is_hydrated(repo, &oid) => {
                lfs = true;
                missing.push(MissingObject {
                    side: side.into(),
                    oid,
                    size,
                });
                continue;
            }
            Some((oid, _)) => {
                lfs = true;
                std::fs::read(lfs_object_path(repo, &oid)).map_err(read_err)?
            }
            None => bytes,
        };
        let name = match side {
            "base" => "the common version",
            other => other,
        };
        if is_binary(&bytes) {
            return Ok(Read::Unavailable(format!(
                "{name} is saved as binary (Asset Serialization isn't Force Text)"
            )));
        }
        let Ok(text) = String::from_utf8(bytes) else {
            return Ok(Read::Unavailable(format!("{name} isn't UTF-8 text")));
        };
        if !text.is_empty() && !text.contains("--- !u!") {
            return Ok(Read::Unavailable(format!("{name} has no Unity objects")));
        }
        texts.push(text);
    }
    if !missing.is_empty() {
        return Ok(Read::NeedsFetch(missing));
    }
    let theirs = texts.pop().unwrap_or_default();
    let ours = texts.pop().unwrap_or_default();
    let base = texts.pop().unwrap_or_default();
    Ok(Read::Ready(Versions {
        base,
        ours,
        theirs,
        lfs,
    }))
}

fn ready(repo: &Repo, path: &str) -> Result<Versions, Error> {
    match read(repo, path)? {
        Read::Ready(v) => Ok(v),
        Read::NeedsFetch(_) => Err(Error::Refused(format!(
            "{path}: download its LFS versions first"
        ))),
        Read::Unavailable(reason) => Err(Error::Refused(format!("{path}: {reason}"))),
    }
}

/// What merging conflicted `path` by property would do, or why it can't.
///
/// # Errors
/// [`Error::Read`] when the index or an LFS object can't be read.
pub fn offer(repo: &Repo, path: &str) -> Result<MergeOffer, Error> {
    let v = match read(repo, path)? {
        Read::Ready(v) => v,
        Read::NeedsFetch(missing) => return Ok(MergeOffer::NeedsFetch { missing }),
        Read::Unavailable(reason) => return Ok(MergeOffer::Unavailable { reason }),
    };
    match analyze(path, &v.base, &v.ours, &v.theirs, &repo_guid_index(repo)) {
        Ok(m) => Ok(MergeOffer::Ready {
            yours: m.yours,
            theirs: m.theirs,
            clashes: m.clashes,
            objects: m.objects,
        }),
        Err(Error::Refused(reason)) => Ok(MergeOffer::Unavailable { reason }),
        Err(e) => Err(e),
    }
}

/// Every change in conflicted `path` (R9's table).
///
/// # Errors
/// [`Error::Refused`] when it can't be merged by property.
pub fn conflict_merge(repo: &Repo, path: &str) -> Result<PropertyMerge, Error> {
    let v = ready(repo, path)?;
    analyze(path, &v.base, &v.ours, &v.theirs, &repo_guid_index(repo))
}

/// Conflicted `path` merged with `picks`, and whether it's stored in LFS (then the text
/// is what the work tree holds, not what git stores).
///
/// # Errors
/// [`Error::Refused`] when it can't be merged by property or a pick is stale.
pub fn conflict_merged(
    repo: &Repo,
    path: &str,
    picks: &[RowPick],
) -> Result<(MergedFile, bool), Error> {
    let v = ready(repo, path)?;
    let merged = merge(
        path,
        &v.base,
        &v.ours,
        &v.theirs,
        picks,
        &repo_guid_index(repo),
    )?;
    Ok((merged, v.lfs))
}
