use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};

use gix::bstr::BStr;
use serde::{Deserialize, Serialize};

use crate::cli::{GitCli, command_error, failure_message};
use crate::diff::is_binary;
use crate::lfs::{lfs_is_hydrated, parse_lfs_pointer};
use crate::repo::{Error, Repo, read_err};
use crate::unity::{UnityMergeFile, run_smart_merge_on_conflict};

/// Unity files Smart Merge handles (R4 state 4).
const UNITY_YAML: [&str; 7] = [
    "unity",
    "prefab",
    "asset",
    "mat",
    "anim",
    "controller",
    "overrideController",
];

/// Conflict markers this long never occur in real files, so content that
/// itself contains `<<<<<<<` can't confuse the parser.
const MARKER: usize = 31;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ConflictKind {
    UnityYaml,
    Code,
    /// LFS or other binary content, or one side deleted the file: pick a side.
    Binary,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Conflict {
    pub path: String,
    pub kind: ConflictKind,
    /// Staged since the merge stopped.
    pub resolved: bool,
}

/// A merge that stopped on conflicts and waits to be committed or aborted.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MergeState {
    /// The branch being merged in, when gitgud started the merge.
    pub theirs: Option<String>,
    /// The prefilled commit message (git's `MERGE_MSG` without comment lines).
    pub message: String,
    /// Every file the merge stopped on, in list order (kind, then path).
    pub conflicts: Vec<Conflict>,
    /// Unity Smart Merge per-property verification summary (v1.0).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub unity_verify: Vec<UnityMergeFile>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum MergeOutcome {
    /// Merged and committed (or fast-forwarded).
    Merged,
    /// Stopped on conflicts; see [`Repo::merge_state`].
    Conflicts,
}

/// Side of a conflict to keep.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Side {
    Yours,
    Theirs,
}

/// The three versions of a conflicted text file, as git's merge chunks them.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConflictText {
    pub chunks: Vec<Chunk>,
    /// Whether the merged file ends with a newline.
    pub final_newline: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum Chunk {
    /// Lines both sides agree on (or one side changed cleanly).
    Same { lines: Vec<String> },
    Conflict {
        yours: Vec<String>,
        base: Vec<String>,
        theirs: Vec<String>,
    },
}

/// One side of a conflict as the side picker shows it; `None` sides were deleted.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SideInfo {
    /// Bytes of the real content (an LFS pointer's `size`).
    pub size: u64,
    pub lfs: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub oid: Option<String>,
    pub hydrated: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ConflictSides {
    pub yours: Option<SideInfo>,
    pub theirs: Option<SideInfo>,
}

/// What gitgud remembers about a merge it started: `.git/gitgud/merge.json`.
#[derive(Debug, Serialize, Deserialize)]
struct Started {
    /// The merge this describes; a different `MERGE_HEAD` means it ended outside gitgud.
    #[serde(default)]
    merge_head: String,
    theirs: String,
    conflicts: Vec<Conflict>,
    #[serde(default)]
    unity_verify: Vec<UnityMergeFile>,
}

impl Repo {
    /// Files that merging `branch` into HEAD would leave conflicted, without
    /// touching anything (`git merge-tree`).
    ///
    /// # Errors
    /// [`Error::Command`] when git can't compute the merge.
    pub fn predict_merge(&self, git: &GitCli, branch: &str) -> Result<Vec<String>, Error> {
        let args = [
            "merge-tree",
            "--write-tree",
            "--name-only",
            "--no-messages",
            "-z",
            "HEAD",
            branch,
        ];
        let (code, out, err) = git.run_status(self.root(), &args, None)?;
        match code {
            0 => Ok(Vec::new()),
            // First record is the tree id; then the conflicted paths.
            1 => Ok(out
                .split('\0')
                .skip(1)
                .filter(|p| !p.is_empty())
                .map(str::to_owned)
                .collect()),
            _ => Err(command_error(&args, failure_message(&out, &err))),
        }
    }

    /// Files a merge of `branch` may rewrite: what changed on its side since the
    /// merge base. The op log snapshots exactly these.
    ///
    /// # Errors
    /// [`Error::Command`] for an unknown branch or unrelated histories.
    pub fn merge_paths(&self, git: &GitCli, branch: &str) -> Result<Vec<String>, Error> {
        let range = format!("HEAD...{branch}");
        let out = git.run(
            self.root(),
            &["diff", "--name-only", "--no-renames", "-z", &range],
            None,
        )?;
        Ok(out
            .split('\0')
            .filter(|p| !p.is_empty())
            .map(str::to_owned)
            .collect())
    }

    /// Merges `branch` into the current branch. A clean merge commits at once
    /// (hooks run); a conflicted one stops with the files to resolve.
    ///
    /// # Errors
    /// [`Error::Refused`] when a merge is already in progress, or git's error
    /// (local changes in the way, unknown branch).
    pub fn merge(&self, git: &GitCli, branch: &str) -> Result<MergeOutcome, Error> {
        if self.rebasing() {
            return Err(Error::Refused("finish or abort the rebase first".into()));
        }
        if self.sequencer_head()?.is_some() {
            return Err(Error::Refused(
                "finish or abort the operation in progress first".into(),
            ));
        }
        if self.merge_head()?.is_some() {
            return Err(Error::Refused("a merge is already in progress".into()));
        }
        let args = ["merge", "--no-edit", "--no-stat", branch];
        let (code, out, err) = git.run_status(self.root(), &args, None)?;
        if code == 0 {
            return Ok(MergeOutcome::Merged);
        }
        let Some(merge_head) = self.merge_head()? else {
            // Refused before starting: nothing changed.
            return Err(command_error(&args, failure_message(&out, &err)));
        };
        let paths = self.conflicted_paths()?;
        if paths.is_empty() {
            // Merged cleanly but a hook refused the commit: put things back and say why.
            git.run(self.root(), &["merge", "--abort"], None)?;
            return Err(command_error(&args, failure_message(&out, &err)));
        }
        let conflicts = self.classify(git, &paths)?;
        let started = Started {
            merge_head,
            theirs: branch.to_owned(),
            conflicts,
            unity_verify: Vec::new(),
        };
        fs::create_dir_all(self.state_file().parent().expect("has a parent")).map_err(read_err)?;
        fs::write(
            self.state_file(),
            serde_json::to_vec(&started).map_err(read_err)?,
        )
        .map_err(read_err)?;
        Ok(MergeOutcome::Conflicts)
    }

    /// The merge in progress, if any (started by gitgud or not).
    ///
    /// # Errors
    /// [`Error::Read`] when the index or git's merge files can't be read.
    pub fn merge_state(&self, git: &GitCli) -> Result<Option<MergeState>, Error> {
        let Some(merge_head) = self.merge_head()? else {
            return Ok(None);
        };
        let now: Vec<String> = self.conflicted_paths()?;
        let started: Option<Started> = fs::read(self.state_file())
            .ok()
            .map(|b| serde_json::from_slice::<Started>(&b).map_err(read_err))
            .transpose()?
            .filter(|s| s.merge_head == merge_head);
        let mut conflicts = match &started {
            Some(s) => s.conflicts.clone(),
            None => self.classify(git, &now)?,
        };
        for c in &mut conflicts {
            c.resolved = !now.contains(&c.path);
        }
        let (theirs, unity_verify) = match started {
            Some(s) => (Some(s.theirs), s.unity_verify),
            None => (None, Vec::new()),
        };
        let msg = fs::read_to_string(self.git_dir().join("MERGE_MSG")).unwrap_or_default();
        let message = msg
            .lines()
            .filter(|l| !l.starts_with('#'))
            .collect::<Vec<_>>()
            .join("\n")
            .trim()
            .to_owned();
        Ok(Some(MergeState {
            theirs,
            message,
            conflicts,
            unity_verify,
        }))
    }

    /// Gives up the merge in progress: back to HEAD as it was (`git merge --abort`).
    ///
    /// # Errors
    /// [`Error::Command`] with git's message.
    pub fn merge_abort(&self, git: &GitCli) -> Result<(), Error> {
        git.run(self.root(), &["merge", "--abort"], None)?;
        self.forget_merge()
    }

    /// Commits the resolved merge with `message`.
    ///
    /// # Errors
    /// [`Error::Refused`] while files are still conflicted or Smart Merge items
    /// wait for confirmation, or git's error.
    pub fn merge_commit(&self, git: &GitCli, message: &str) -> Result<(), Error> {
        let left = self.conflicted_paths()?;
        if !left.is_empty() {
            return Err(Error::Refused(format!(
                "{} file(s) still conflicted",
                left.len()
            )));
        }
        if let Some(merge_head) = self.merge_head()?
            && let Some(started) = fs::read(self.state_file())
                .ok()
                .and_then(|b| serde_json::from_slice::<Started>(&b).ok())
                .filter(|s| s.merge_head == merge_head)
        {
            let unconfirmed: usize = started
                .unity_verify
                .iter()
                .flat_map(|f| &f.verify)
                .filter(|v| !v.confirmed)
                .count();
            if unconfirmed > 0 {
                return Err(Error::Refused(format!(
                    "{unconfirmed} Smart Merge change(s) still need verification before committing"
                )));
            }
        }
        self.commit(git, message)?;
        self.forget_merge()
    }

    /// Runs the configured Unity Smart Merge driver over unresolved Unity YAML conflicts,
    /// recording per-property verification items into `.git/gitgud/merge.json`.
    ///
    /// # Errors
    /// [`Error::Refused`] if no merge is in progress, or [`Error::Read`] / [`Error::Command`].
    pub fn remerge_unity_files(&self, git: &GitCli) -> Result<usize, Error> {
        let Some(merge_head) = self.merge_head()? else {
            return Err(Error::Refused("no merge is in progress".into()));
        };
        let now = self.conflicted_paths()?;
        let mut started = fs::read(self.state_file())
            .ok()
            .and_then(|b| serde_json::from_slice::<Started>(&b).ok())
            .filter(|s| s.merge_head == merge_head)
            .unwrap_or(Started {
                merge_head: merge_head.clone(),
                theirs: String::new(),
                conflicts: self.classify(git, &now)?,
                unity_verify: Vec::new(),
            });

        let mut merged_count = 0usize;
        let unity_paths: Vec<String> = started
            .conflicts
            .iter()
            .filter(|c| c.kind == ConflictKind::UnityYaml && now.contains(&c.path))
            .map(|c| c.path.clone())
            .collect();

        for path in unity_paths {
            let [base, ours, theirs] = self.stages(&path)?;
            let (Some(ours), Some(theirs)) = (ours, theirs) else {
                continue;
            };
            let base = base.unwrap_or_default();
            if let Some(umf) = run_smart_merge_on_conflict(git, self, &path, &base, &ours, &theirs)?
            {
                started.unity_verify.retain(|f| f.path != path);
                if !umf.verify.is_empty() || !umf.auto_merged.is_empty() {
                    started.unity_verify.push(umf);
                }
                merged_count += 1;
            }
        }

        fs::create_dir_all(self.state_file().parent().expect("has a parent")).map_err(read_err)?;
        fs::write(
            self.state_file(),
            serde_json::to_vec(&started).map_err(read_err)?,
        )
        .map_err(read_err)?;
        Ok(merged_count)
    }

    /// Marks Smart Merge verification items confirmed or unconfirmed on `path`.
    ///
    /// # Errors
    /// [`Error::Refused`] when no matching merge or verification entry exists.
    pub fn confirm_unity_verify(
        &self,
        path: &str,
        file_id: Option<&str>,
        property: Option<&str>,
        confirmed: bool,
    ) -> Result<(), Error> {
        let Some(merge_head) = self.merge_head()? else {
            return Err(Error::Refused("no merge is in progress".into()));
        };
        let bytes = fs::read(self.state_file()).map_err(|_| {
            Error::Refused("no Smart Merge verification data for this merge".into())
        })?;
        let mut started: Started = serde_json::from_slice(&bytes).map_err(read_err)?;
        if started.merge_head != merge_head {
            return Err(Error::Refused("merge state is stale".into()));
        }
        let mut updated = 0usize;
        for file in &mut started.unity_verify {
            if file.path != path {
                continue;
            }
            for item in &mut file.verify {
                let id_match = file_id.is_none_or(|id| item.file_id == id);
                let prop_match = property.is_none_or(|p| item.property == p);
                if id_match && prop_match {
                    item.confirmed = confirmed;
                    updated += 1;
                }
            }
        }
        if updated == 0 {
            return Err(Error::Refused(format!(
                "no Smart Merge verification item matched {path}"
            )));
        }
        fs::write(
            self.state_file(),
            serde_json::to_vec(&started).map_err(read_err)?,
        )
        .map_err(read_err)
    }

    /// The conflicted text file `path` chunked by git's own merge (`merge-file
    /// --diff3` over the three index stages).
    ///
    /// # Errors
    /// [`Error::Refused`] when `path` isn't a conflicted text file with both sides.
    pub fn conflict_text(&self, git: &GitCli, path: &str) -> Result<ConflictText, Error> {
        let [base, yours, theirs] = self.stages(path)?;
        let (Some(yours), Some(theirs)) = (yours, theirs) else {
            return Err(Error::Refused(format!(
                "{path} was deleted on one side: pick a side"
            )));
        };
        let base = base.unwrap_or_default();
        if is_binary(&yours) || is_binary(&theirs) || is_binary(&base) {
            return Err(Error::Refused(format!("{path} is binary: pick a side")));
        }
        let dir = self.git_dir().join("gitgud").join("merge-file");
        fs::create_dir_all(&dir).map_err(read_err)?;
        let files =
            [("yours", &yours), ("base", &base), ("theirs", &theirs)].map(|(name, data)| {
                let p = dir.join(name);
                fs::write(&p, data).map(|()| p)
            });
        let [y, b, t] = files;
        let (y, b, t) = (
            y.map_err(read_err)?,
            b.map_err(read_err)?,
            t.map_err(read_err)?,
        );
        let marker = format!("--marker-size={MARKER}");
        let args = [
            "merge-file",
            "-p",
            "--diff3",
            marker.as_str(),
            "-L",
            "yours",
            "-L",
            "base",
            "-L",
            "theirs",
            path_str(&y)?,
            path_str(&b)?,
            path_str(&t)?,
        ];
        let (code, out, err) = git.run_status(self.root(), &args, None)?;
        if !(0..=127).contains(&code) {
            return Err(command_error(&args, failure_message(&out, &err)));
        }
        Ok(ConflictText {
            final_newline: out.ends_with('\n'),
            chunks: parse_merge_file(&out),
        })
    }

    /// Resolves `path` with `content` already in work-tree form (an LFS file's own bytes),
    /// written as is and staged, so only git's clean filter runs.
    ///
    /// # Errors
    /// [`Error::Read`] / [`Error::Command`] when writing or staging fails.
    pub fn resolve_with_worktree(
        &self,
        git: &GitCli,
        path: &str,
        content: &str,
    ) -> Result<(), Error> {
        self.stages(path)?;
        fs::write(self.root().join(path), content).map_err(read_err)?;
        git.run(
            self.root(),
            &["--literal-pathspecs", "add", "--", path],
            None,
        )
        .map(drop)
    }

    /// Resolves `path` with `content` (git form, `\n` line ends): written through
    /// the work-tree filters (autocrlf...) and staged.
    ///
    /// # Errors
    /// [`Error::Read`] / [`Error::Command`] when writing or staging fails.
    pub fn resolve_with(&self, git: &GitCli, path: &str, content: &str) -> Result<(), Error> {
        use gix::filter::plumbing::driver::apply::Delay;
        use gix::filter::plumbing::pipeline::convert::to_worktree::Options;
        // Only conflicted index paths: never a path outside the work tree.
        self.stages(path)?;
        let repo = self.gix();
        let (mut pipeline, _index) = repo.filter_pipeline(None).map_err(read_err)?;
        let options = Options {
            can_delay: Delay::Forbid,
            ..Options::default()
        };
        let mut converted = pipeline
            .convert_to_worktree(content.as_bytes(), BStr::new(path), options)
            .map_err(read_err)?;
        let mut out = Vec::new();
        converted.read_to_end(&mut out).map_err(read_err)?;
        fs::write(self.root().join(path), out).map_err(read_err)?;
        git.run(
            self.root(),
            &["--literal-pathspecs", "add", "--", path],
            None,
        )
        .map(drop)
    }

    /// Resolves `path` by keeping one side whole (a deletion stays a deletion).
    ///
    /// # Errors
    /// [`Error::Command`] with git's message.
    pub fn resolve_side(&self, git: &GitCli, path: &str, side: Side) -> Result<(), Error> {
        let [_, yours, theirs] = self.stages(path)?;
        let kept = match side {
            Side::Yours => yours,
            Side::Theirs => theirs,
        };
        let root = self.root();
        if kept.is_none() {
            return git
                .run(root, &["--literal-pathspecs", "rm", "-q", "--", path], None)
                .map(drop);
        }
        let flag = match side {
            Side::Yours => "--ours",
            Side::Theirs => "--theirs",
        };
        git.run(
            root,
            &["--literal-pathspecs", "checkout", flag, "--", path],
            None,
        )?;
        git.run(root, &["--literal-pathspecs", "add", "--", path], None)
            .map(drop)
    }

    /// Size and LFS hydration metadata of each side of conflicted `path`, for picking one.
    ///
    /// # Errors
    /// [`Error::Refused`] when `path` isn't conflicted.
    pub fn conflict_sides(&self, path: &str) -> Result<ConflictSides, Error> {
        let [_, yours, theirs] = self.stages(path)?;
        Ok(ConflictSides {
            yours: yours.as_deref().map(|b| side_info(self, b)),
            theirs: theirs.as_deref().map(|b| side_info(self, b)),
        })
    }

    pub(crate) fn git_dir(&self) -> PathBuf {
        self.gix().git_dir().to_owned()
    }

    fn state_file(&self) -> PathBuf {
        self.git_dir().join("gitgud").join("merge.json")
    }

    fn forget_merge(&self) -> Result<(), Error> {
        match fs::remove_file(self.state_file()) {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(read_err(e)),
            _ => Ok(()),
        }
    }

    fn merge_head(&self) -> Result<Option<String>, Error> {
        match fs::read_to_string(self.git_dir().join("MERGE_HEAD")) {
            Ok(s) => Ok(Some(s.trim().to_owned())),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(read_err(e)),
        }
    }

    /// Paths with conflict stages in the index, sorted.
    pub(crate) fn conflicted_paths(&self) -> Result<Vec<String>, Error> {
        let index = self.gix().index_or_empty().map_err(read_err)?;
        let mut paths: Vec<String> = index
            .entries()
            .iter()
            .filter(|e| e.stage_raw() != 0)
            .map(|e| e.path(&index).to_string())
            .collect();
        paths.dedup();
        Ok(paths)
    }

    /// Base, yours and theirs (stages 1–3) of a conflicted path; `None` where absent.
    pub(crate) fn stages(&self, path: &str) -> Result<[Option<Vec<u8>>; 3], Error> {
        let repo = self.gix();
        let index = repo.index_or_empty().map_err(read_err)?;
        let mut out = [None, None, None];
        for e in index.entries() {
            let stage = e.stage_raw();
            if (1..=3).contains(&stage) && e.path(&index) == path {
                let blob = repo.find_blob(e.id).map_err(read_err)?;
                out[stage as usize - 1] = Some(blob.data.clone());
            }
        }
        if out.iter().all(Option::is_none) {
            return Err(Error::Refused(format!("{path} is not conflicted")));
        }
        Ok(out)
    }

    pub(crate) fn classify(&self, git: &GitCli, paths: &[String]) -> Result<Vec<Conflict>, Error> {
        let lfs = self.lfs_paths(git, paths)?;
        let mut conflicts = Vec::with_capacity(paths.len());
        for path in paths {
            let [_, yours, theirs] = self.stages(path)?;
            let one_side_deleted = yours.is_none() || theirs.is_none();
            let binary =
                yours.as_deref().is_some_and(is_binary) || theirs.as_deref().is_some_and(is_binary);
            let unity = Path::new(path)
                .extension()
                .and_then(|e| e.to_str())
                .is_some_and(|e| UNITY_YAML.contains(&e));
            let kind = if lfs.contains(path) || binary || one_side_deleted {
                ConflictKind::Binary
            } else if unity {
                ConflictKind::UnityYaml
            } else {
                ConflictKind::Code
            };
            conflicts.push(Conflict {
                path: path.clone(),
                kind,
                resolved: false,
            });
        }
        conflicts.sort_by(|a, b| {
            kind_order(a.kind)
                .cmp(&kind_order(b.kind))
                .then(a.path.cmp(&b.path))
        });
        Ok(conflicts)
    }

    /// Which of `paths` git stores through LFS (`filter=lfs` in .gitattributes).
    fn lfs_paths(&self, git: &GitCli, paths: &[String]) -> Result<Vec<String>, Error> {
        if paths.is_empty() {
            return Ok(Vec::new());
        }
        let out = git.run(
            self.root(),
            &["check-attr", "-z", "--stdin", "filter"],
            Some(paths.join("\0").as_bytes()),
        )?;
        // Records: path \0 attribute \0 value \0
        let fields: Vec<&str> = out.split('\0').collect();
        Ok(fields
            .chunks(3)
            .filter(|r| r.len() == 3 && r[2] == "lfs")
            .map(|r| r[0].to_owned())
            .collect())
    }
}

/// Unity first, then code, then binaries (R4-A list order).
fn kind_order(kind: ConflictKind) -> u8 {
    match kind {
        ConflictKind::UnityYaml => 0,
        ConflictKind::Code => 1,
        ConflictKind::Binary => 2,
    }
}

fn side_info(repo: &Repo, data: &[u8]) -> SideInfo {
    if let Some((oid, size)) = parse_lfs_pointer(data) {
        let hydrated = lfs_is_hydrated(repo, &oid);
        return SideInfo {
            size,
            lfs: true,
            oid: Some(oid),
            hydrated,
        };
    }
    SideInfo {
        size: data.len() as u64,
        lfs: false,
        oid: None,
        hydrated: true,
    }
}

fn path_str(p: &Path) -> Result<&str, Error> {
    p.to_str()
        .ok_or_else(|| Error::Read(format!("{} is not UTF-8", p.display())))
}

/// Splits `git merge-file --diff3` output (with [`MARKER`]-long markers) into
/// chunks. Line texts lose their line ending.
pub fn parse_merge_file(text: &str) -> Vec<Chunk> {
    let open = "<".repeat(MARKER);
    let base_mark = "|".repeat(MARKER);
    let mid = "=".repeat(MARKER);
    let close = ">".repeat(MARKER);
    let is = |line: &str, m: &str| line == m || line.starts_with(&format!("{m} "));

    let mut chunks = Vec::new();
    let mut same: Vec<String> = Vec::new();
    let mut lines = text.lines();
    while let Some(line) = lines.next() {
        if !is(line, &open) {
            same.push(line.to_owned());
            continue;
        }
        if !same.is_empty() {
            chunks.push(Chunk::Same {
                lines: std::mem::take(&mut same),
            });
        }
        let (mut yours, mut base, mut theirs) = (Vec::new(), Vec::new(), Vec::new());
        let mut part = 0;
        for l in lines.by_ref() {
            if is(l, &base_mark) {
                part = 1;
            } else if is(l, &mid) {
                part = 2;
            } else if is(l, &close) {
                break;
            } else {
                [&mut yours, &mut base, &mut theirs][part].push(l.to_owned());
            }
        }
        chunks.push(Chunk::Conflict {
            yours,
            base,
            theirs,
        });
    }
    if !same.is_empty() {
        chunks.push(Chunk::Same { lines: same });
    }
    chunks
}
