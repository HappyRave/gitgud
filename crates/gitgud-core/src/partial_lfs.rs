use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::cli::GitCli;
use crate::lfs::{lfs_is_hydrated, lfs_object_path, parse_lfs_pointer};
use crate::repo::{Error, Repo, read_err};
use crate::task::{Cancel, Progress};
use crate::unity::find_unity_root;
use crate::unity_yaml::repo_guid_index;

/// What a partial repo keeps downloaded (R10-B), saved in `.git/gitgud/lfs-partial.json`.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PartialState {
    /// Follow the scenes enabled in Build Settings.
    pub build_scenes: bool,
    /// More scenes or prefabs to keep working on, repo paths.
    pub scenes: Vec<String>,
    /// Files and folders downloaded on request, kept until removed.
    pub kept: Vec<String>,
}

impl PartialState {
    /// The default for a repo made partial: the build's scenes.
    #[must_use]
    pub fn build() -> Self {
        Self {
            build_scenes: true,
            ..Self::default()
        }
    }
}

/// Bytes and files of one share of the LFS content.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Share {
    pub bytes: u64,
    pub files: usize,
}

impl Share {
    fn add(&mut self, bytes: u64) {
        self.bytes += bytes;
        self.files += 1;
    }
}

/// An LFS file the plan doesn't download.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SkippedFile {
    pub path: String,
    pub bytes: u64,
    /// Downloaded in the work tree now (applying turns it back into a pointer).
    pub on_disk: bool,
}

/// A folder holding LFS files the plan doesn't download.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SkippedFolder {
    pub folder: String,
    pub skipped: Share,
    /// Every LFS file in the folder, downloaded or not.
    pub total: Share,
}

/// What a [`PartialState`] downloads in a repo, and what it leaves out.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LfsPlan {
    /// Whether the repo is partial now (a state is saved).
    pub partial: bool,
    pub state: PartialState,
    /// The scenes enabled in Build Settings.
    pub build_scenes: Vec<String>,
    /// Every scene in the repo, to pick more from.
    pub all_scenes: Vec<String>,
    /// LFS files at HEAD.
    pub total: Share,
    /// Reached from the scenes.
    pub used: Share,
    /// Loaded by Unity without a reference: code, plugins, `Resources`, `Editor`, outside `Assets/`.
    pub always: Share,
    /// Kept on request, beyond the above.
    pub kept: Share,
    pub skipped: Share,
    /// The part of `skipped` downloaded in the work tree now: what applying frees.
    pub skipped_on_disk: Share,
    /// Largest first.
    pub skipped_folders: Vec<SkippedFolder>,
    /// Every skipped file, by path, for browsing inside [`Self::skipped_folders`].
    pub skipped_files: Vec<SkippedFile>,
    /// Downloaded in the work tree now.
    pub on_disk: Share,
    /// Chosen scenes or kept paths that aren't in the repo.
    pub missing: Vec<String>,
    /// LFS-stored scenes and prefabs downloaded to follow their references.
    pub fetched_to_read: usize,
}

/// One LFS file at HEAD, from `git lfs ls-files --json`.
#[derive(Debug, Clone, Deserialize)]
struct LfsFile {
    name: String,
    size: u64,
    /// Its content is in the work tree (not a pointer).
    checkout: bool,
}

#[derive(Deserialize)]
struct LsFiles {
    files: Option<Vec<LfsFile>>,
}

const STATE_FILE: &str = "gitgud/lfs-partial.json";
/// Unity YAML and JSON assets whose GUID references are followed.
const REFERENCING: &[&str] = &[
    "unity",
    "prefab",
    "mat",
    "asset",
    "controller",
    "overridecontroller",
    "anim",
    "mask",
    "spriteatlas",
    "spriteatlasv2",
    "playable",
    "signal",
    "lighting",
    "shadergraph",
    "shadersubgraph",
    "physicmaterial",
    "physicsmaterial2d",
    "terrainlayer",
    "mixer",
    "brush",
    "preset",
    "guiskin",
    "fontsettings",
    "flare",
    "rendertexture",
    "cubemap",
    "vfx",
    "vfxoperator",
    "vfxblock",
    "inputactions",
    "asmdef",
];
/// Loaded by Unity without a GUID reference.
const ALWAYS_DIRS: &[&str] = &[
    "Resources",
    "Editor",
    "StreamingAssets",
    "Gizmos",
    "Editor Default Resources",
    "AddressableAssetsData",
];
const ALWAYS_EXTS: &[&str] = &["dll", "so", "bundle", "jar", "aar", "a", "dylib", "cs"];
/// Folders listed in [`LfsPlan::skipped_folders`].
const SKIPPED_LISTED: usize = 60;
/// Longest pattern list passed on one command line (Windows caps it near 32k).
const ARG_BUDGET: usize = 24_000;

/// The saved state of repo `repo`, when it is partial.
///
/// # Errors
/// [`Error::Read`] when the state file can't be read or parsed.
pub fn load_state(repo: &Repo) -> Result<Option<PartialState>, Error> {
    let file = repo.git_dir().join(STATE_FILE);
    match std::fs::read(&file) {
        Ok(bytes) => serde_json::from_slice(&bytes)
            .map(Some)
            .map_err(|e| Error::Read(format!("{}: {e}", file.display()))),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(read_err(e)),
    }
}

fn save_state(repo: &Repo, state: Option<&PartialState>) -> Result<(), Error> {
    let file = repo.git_dir().join(STATE_FILE);
    match state {
        Some(s) => {
            if let Some(dir) = file.parent() {
                std::fs::create_dir_all(dir).map_err(read_err)?;
            }
            let json = serde_json::to_vec_pretty(s).expect("state serializes");
            std::fs::write(&file, json).map_err(read_err)
        }
        None => match std::fs::remove_file(&file) {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(read_err(e)),
            _ => Ok(()),
        },
    }
}

fn lfs_files(git: &GitCli, repo: &Repo) -> Result<Vec<LfsFile>, Error> {
    let (code, out, err) = git.run_status(repo.root(), &["lfs", "ls-files", "--json"], None)?;
    if code != 0 {
        // An unborn HEAD has no LFS files yet.
        if repo.gix().head_commit().is_err() {
            return Ok(Vec::new());
        }
        return Err(Error::Command {
            command: "git lfs ls-files --json".into(),
            message: err.trim().to_owned(),
        });
    }
    let parsed: LsFiles = serde_json::from_str(&out)
        .map_err(|e| Error::Read(format!("git lfs ls-files --json: {e}")))?;
    Ok(parsed.files.unwrap_or_default())
}

/// The scenes enabled in `ProjectSettings/EditorBuildSettings.asset`, as repo paths.
fn build_scenes(repo: &Repo, prefix: &str) -> Vec<String> {
    let Some(project) = find_unity_root(repo.root()) else {
        return Vec::new();
    };
    let Ok(text) =
        std::fs::read_to_string(project.join("ProjectSettings/EditorBuildSettings.asset"))
    else {
        return Vec::new();
    };
    let mut out = Vec::new();
    let mut enabled = false;
    for line in text.lines() {
        let t = line.trim_start().trim_start_matches("- ");
        if let Some(v) = t.strip_prefix("enabled:") {
            enabled = v.trim() == "1";
        } else if let Some(p) = t.strip_prefix("path:")
            && enabled
        {
            let p = p.trim();
            if !p.is_empty() {
                out.push(format!("{prefix}{p}"));
            }
        }
    }
    out
}

/// The Unity project's folder inside the repo, as a path prefix (`""` or `"Game/"`).
fn project_prefix(repo: &Repo) -> Option<String> {
    let project = find_unity_root(repo.root())?;
    let rel = project.strip_prefix(repo.root()).ok()?;
    let rel = rel.to_string_lossy().replace('\\', "/");
    Some(if rel.is_empty() {
        rel
    } else {
        format!("{rel}/")
    })
}

fn extension(path: &str) -> String {
    Path::new(path)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase()
}

/// Whether Unity loads `path` without anything referencing it.
fn always_needed(path: &str, prefix: &str) -> bool {
    let Some(in_assets) = path
        .strip_prefix(prefix)
        .and_then(|p| p.strip_prefix("Assets/"))
    else {
        return true;
    };
    ALWAYS_EXTS.contains(&extension(path).as_str())
        || in_assets
            .split('/')
            .rev()
            .skip(1)
            .any(|dir| ALWAYS_DIRS.iter().any(|d| d.eq_ignore_ascii_case(dir)))
}

/// Every GUID written in `text`: `guid: …` in YAML, `"guid": "…"` / `m_GUID` in JSON.
fn guids(text: &[u8]) -> Vec<&str> {
    let mut out = Vec::new();
    for needle in [&b"guid"[..], b"GUID"] {
        for found in memchr::memmem::find_iter(text, needle) {
            let mut i = found + 4;
            while i < text.len() && matches!(text[i], b'"' | b'\'' | b' ' | b':' | b'\t') {
                i += 1;
            }
            let end = i + 32;
            if end <= text.len()
                && text[i..end].iter().all(u8::is_ascii_hexdigit)
                && text.get(end).is_none_or(|c| !c.is_ascii_hexdigit())
                && let Ok(g) = std::str::from_utf8(&text[i..end])
            {
                out.push(g);
            }
        }
    }
    out
}

/// Whether `bytes` is text that can name GUIDs: Unity YAML or JSON, not a binary asset.
fn readable(bytes: &[u8]) -> bool {
    let head = bytes.trim_ascii_start();
    head.starts_with(b"%YAML") || head.starts_with(b"{") || head.starts_with(b"fileFormatVersion")
}

/// Downloads the LFS objects of `paths` into the local store, without touching the work tree.
fn fetch_objects(git: &GitCli, repo: &Repo, paths: &[String]) -> Result<(), Error> {
    for chunk in pattern_chunks(paths.iter().map(|p| pattern(p))) {
        git.run(
            repo.root(),
            &["lfs", "fetch", "--include", &chunk, "--exclude", ""],
            None,
        )?;
    }
    Ok(())
}

/// Comma-joined pattern lists, each short enough for one command line.
fn pattern_chunks(patterns: impl Iterator<Item = String>) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    for p in patterns {
        if !cur.is_empty() && cur.len() + p.len() + 1 > ARG_BUDGET {
            out.push(std::mem::take(&mut cur));
        }
        if !cur.is_empty() {
            cur.push(',');
        }
        cur.push_str(&p);
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out
}

/// `path` as a git-lfs include pattern matching it: characters git-lfs would read as
/// wildcards or list separators become `?` (any one character), so the pattern may
/// match a sibling that differs only there, never miss the file.
fn pattern(path: &str) -> String {
    let n = path.chars().count();
    path.chars()
        .enumerate()
        .map(|(i, c)| match c {
            ',' | '*' | '?' | '[' | ']' | '\\' | '!' | '#' if c != '#' || i == 0 => '?',
            ' ' if i + 1 == n => '?',
            _ => c,
        })
        .collect()
}

struct Walk<'a> {
    git: &'a GitCli,
    repo: &'a Repo,
    lfs: &'a HashMap<&'a str, u64>,
    reached: BTreeSet<String>,
    fetched: usize,
}

impl Walk<'_> {
    /// `path`'s content as the walk reads it: an LFS object from the store, `None` when
    /// it is a pointer to an object that isn't there.
    fn read(&self, path: &str) -> Option<Vec<u8>> {
        let bytes = std::fs::read(self.repo.root().join(path)).ok()?;
        match parse_lfs_pointer(&bytes) {
            Some((oid, _)) if lfs_is_hydrated(self.repo, &oid) => {
                std::fs::read(lfs_object_path(self.repo, &oid)).ok()
            }
            Some(_) => None,
            None => Some(bytes),
        }
    }

    /// Follows every reference from `roots`, downloading LFS-stored referencing files
    /// (scenes, prefabs, materials...) as they're reached so their references count too.
    fn run(
        &mut self,
        roots: Vec<String>,
        resolve: &dyn Fn(&str) -> Option<String>,
    ) -> Result<(), Error> {
        let mut todo = roots;
        while !todo.is_empty() {
            let mut waiting = Vec::new();
            while let Some(path) = todo.pop() {
                if !self.reached.insert(path.clone()) {
                    continue;
                }
                let meta = format!("{path}.meta");
                let mut sources = vec![meta];
                if REFERENCING.contains(&extension(&path).as_str()) {
                    sources.push(path.clone());
                }
                for source in sources {
                    match self.read(&source) {
                        Some(text) if !readable(&text) => {}
                        Some(text) => {
                            for g in guids(&text) {
                                if let Some(p) = resolve(g)
                                    && !self.reached.contains(&p)
                                {
                                    todo.push(p);
                                }
                            }
                        }
                        None if self.lfs.contains_key(source.as_str()) => waiting.push(source),
                        None => {}
                    }
                }
            }
            if waiting.is_empty() {
                break;
            }
            fetch_objects(self.git, self.repo, &waiting)?;
            self.fetched += waiting.len();
            // Read again now that the objects are here; one that still isn't stays unread.
            for w in waiting {
                if self.read(&w).is_some() {
                    let path = w.strip_suffix(".meta").unwrap_or(&w).to_owned();
                    self.reached.remove(&path);
                    todo.push(path);
                }
            }
        }
        Ok(())
    }
}

/// The LFS files `state` downloads in `repo`, and the plan's numbers. Downloads the
/// LFS-stored scenes and prefabs the walk reaches (into the store only), since their
/// references decide the rest.
///
/// # Errors
/// [`Error::Command`] when git-lfs fails, [`Error::Read`] on a read failure.
pub fn plan(
    git: &GitCli,
    repo: &Repo,
    state: &PartialState,
) -> Result<(LfsPlan, BTreeSet<String>), Error> {
    let files = lfs_files(git, repo)?;
    let lfs: HashMap<&str, u64> = files.iter().map(|f| (f.name.as_str(), f.size)).collect();
    let prefix = project_prefix(repo).unwrap_or_default();
    let build = build_scenes(repo, &prefix);
    let index = repo_guid_index(repo);
    let resolve = |g: &str| index.resolve_guid(g).map(|p| format!("{prefix}{p}"));
    let exists = |p: &str| repo.root().join(p).exists();

    let mut missing = Vec::new();
    let mut roots: Vec<String> = Vec::new();
    let chosen = state
        .build_scenes
        .then_some(build.iter())
        .into_iter()
        .flatten()
        .chain(state.scenes.iter());
    for s in chosen {
        if exists(s) {
            roots.push(s.clone());
        } else {
            missing.push(s.clone());
        }
    }
    let used_roots = roots.clone();
    let mut walk = Walk {
        git,
        repo,
        lfs: &lfs,
        reached: BTreeSet::new(),
        fetched: 0,
    };
    walk.run(used_roots, &resolve)?;
    let used_set = std::mem::take(&mut walk.reached);

    // What Unity loads anyway, and what was kept, pull in their own references too.
    let kept_paths: Vec<&String> = state.kept.iter().collect();
    for k in &kept_paths {
        if !exists(k) && !lfs.keys().any(|p| under(p, k)) {
            missing.push((*k).clone());
        }
    }
    let always_roots: BTreeSet<String> = files
        .iter()
        .filter(|f| always_needed(&f.name, &prefix))
        .map(|f| f.name.clone())
        .collect();
    let kept_roots: Vec<String> = files
        .iter()
        .filter(|f| kept_paths.iter().any(|k| under(&f.name, k)))
        .map(|f| f.name.clone())
        .collect();
    walk.reached = used_set.clone();
    walk.run(
        always_roots.iter().chain(&kept_roots).cloned().collect(),
        &resolve,
    )?;
    let all = walk.reached;

    let mut p = LfsPlan {
        partial: load_state(repo)?.is_some(),
        state: state.clone(),
        build_scenes: build,
        all_scenes: scenes_in_index(repo)?,
        total: Share::default(),
        used: Share::default(),
        always: Share::default(),
        kept: Share::default(),
        skipped: Share::default(),
        skipped_on_disk: Share::default(),
        skipped_folders: Vec::new(),
        skipped_files: Vec::new(),
        on_disk: Share::default(),
        missing,
        fetched_to_read: walk.fetched,
    };
    let mut folders: BTreeMap<String, (Share, Share)> = BTreeMap::new();
    let mut wanted = BTreeSet::new();
    for f in &files {
        p.total.add(f.size);
        if f.checkout {
            p.on_disk.add(f.size);
        }
        let folder = folder_of(&f.name);
        let entry = folders.entry(folder).or_default();
        entry.1.add(f.size);
        if used_set.contains(&f.name) {
            p.used.add(f.size);
        } else if always_roots.contains(&f.name) {
            p.always.add(f.size);
        } else if all.contains(&f.name) {
            p.kept.add(f.size);
        } else {
            p.skipped.add(f.size);
            if f.checkout {
                p.skipped_on_disk.add(f.size);
            }
            p.skipped_files.push(SkippedFile {
                path: f.name.clone(),
                bytes: f.size,
                on_disk: f.checkout,
            });
            entry.0.add(f.size);
            continue;
        }
        wanted.insert(f.name.clone());
    }
    let mut listed: Vec<SkippedFolder> = folders
        .into_iter()
        .filter(|(_, (s, _))| s.files > 0)
        .map(|(folder, (skipped, total))| SkippedFolder {
            folder,
            skipped,
            total,
        })
        .collect();
    listed.sort_by_key(|f| std::cmp::Reverse(f.skipped.bytes));
    listed.truncate(SKIPPED_LISTED);
    p.skipped_folders = listed;
    Ok((p, wanted))
}

fn scenes_in_index(repo: &Repo) -> Result<Vec<String>, Error> {
    let gix = repo.gix();
    let index = gix.index_or_empty().map_err(read_err)?;
    let mut out: Vec<String> = index
        .entries()
        .iter()
        .map(|e| e.path(&index).to_string())
        .filter(|p| extension(p) == "unity")
        .collect();
    out.dedup();
    Ok(out)
}

/// Whether `path` is `target` or inside folder `target`.
fn under(path: &str, target: &str) -> bool {
    let t = target.trim_end_matches('/');
    path == t || path.strip_prefix(t).is_some_and(|r| r.starts_with('/'))
}

/// The folder a skipped file is counted under: up to three levels (`Assets/Plugins/Pack`).
fn folder_of(path: &str) -> String {
    let parts: Vec<&str> = path.split('/').collect();
    let depth = (parts.len() - 1).min(3);
    if depth == 0 {
        return "(top level)".into();
    }
    parts[..depth].join("/")
}

/// The include patterns for `wanted`: a folder whose every LFS file is wanted becomes one
/// pattern (only below the top level, where a bare name would match at any depth).
fn include_patterns(wanted: &BTreeSet<String>, all: &[String]) -> Vec<String> {
    let mut per_dir: BTreeMap<&str, (usize, usize)> = BTreeMap::new();
    for path in all {
        let w = wanted.contains(path);
        let mut at = path.len();
        while let Some(i) = path[..at].rfind('/') {
            let e = per_dir.entry(&path[..i]).or_default();
            e.0 += 1;
            e.1 += usize::from(w);
            at = i;
        }
    }
    let whole = |dir: &str| per_dir.get(dir).is_some_and(|(n, w)| n == w) && dir.contains('/');
    let mut out = Vec::new();
    let mut taken: Vec<&str> = Vec::new();
    for path in wanted {
        let mut top = None;
        let mut at = path.len();
        while let Some(i) = path[..at].rfind('/') {
            if whole(&path[..i]) {
                top = Some(&path[..i]);
            }
            at = i;
        }
        match top {
            Some(dir) if taken.last() == Some(&dir) => {}
            Some(dir) => {
                taken.push(dir);
                out.push(pattern(dir));
            }
            None => out.push(pattern(path)),
        }
    }
    out
}

/// Saves `state`, points git-lfs at what it downloads, downloads it, and turns files it no
/// longer needs back into pointers where that loses nothing (unmodified, object kept in
/// `.git/lfs`). `None` makes the repo whole again: every LFS file downloaded.
///
/// # Errors
/// [`Error::Refused`] while Unity has the project open and files would turn into pointers;
/// [`Error::Cancelled`]; git's error.
pub fn apply(
    git: &GitCli,
    repo: &Repo,
    state: Option<&PartialState>,
    on_progress: &mut dyn FnMut(Progress),
    cancel: &Cancel,
) -> Result<LfsPlan, Error> {
    let Some(state) = state else {
        set_fetchinclude(git, repo, None)?;
        save_state(repo, None)?;
        git.run_with_progress(repo.root(), &["lfs", "pull"], on_progress, cancel)?;
        return Ok(plan(git, repo, &PartialState::build())?.0);
    };
    let (planned, wanted) = plan(git, repo, state)?;
    let files = lfs_files(git, repo)?;
    if files.is_empty() {
        // Nothing in LFS: nothing to choose, and no config to leave behind.
        return Ok(planned);
    }
    let names: Vec<String> = files.iter().map(|f| f.name.clone()).collect();
    let dehydrate: Vec<&LfsFile> = files
        .iter()
        .filter(|f| f.checkout && !wanted.contains(&f.name))
        .collect();
    if !dehydrate.is_empty()
        && find_unity_root(repo.root()).is_some_and(|p| p.join("Temp/UnityLockfile").exists())
    {
        return Err(Error::Refused(format!(
            "close Unity first: {} downloaded files would turn back into pointers under it",
            dehydrate.len()
        )));
    }
    let include = include_patterns(&wanted, &names).join(",");
    save_state(repo, Some(state))?;
    // An empty include list would mean "everything" to git-lfs.
    let value = if include.is_empty() {
        "gitgud-nothing-wanted"
    } else {
        &include
    };
    set_fetchinclude(git, repo, Some(value))?;
    git.run_with_progress(repo.root(), &["lfs", "pull"], on_progress, cancel)?;
    to_pointers(git, repo, &dehydrate)?;
    Ok(plan(git, repo, state)?.0)
}

/// Where `lfs.fetchinclude` lives, relative to the folder of `.git/config` that
/// includes it: the list can pass Windows' 32k command-line limit, so it is
/// written to a file, never passed to `git config`.
const INCLUDE_CONFIG: &str = "gitgud/lfs-include.config";

/// Sets `lfs.fetchinclude` to `value`, or removes it for `None` (every file wanted).
fn set_fetchinclude(git: &GitCli, repo: &Repo, value: Option<&str>) -> Result<(), Error> {
    let root = repo.root();
    // A value an older gitgud wrote straight into .git/config would win over the include.
    config_unset(git, root, &["--unset-all", "lfs.fetchinclude"])?;
    let file = repo.gix().common_dir().join(INCLUDE_CONFIG);
    let pattern = format!("^{}$", INCLUDE_CONFIG.replace('.', "\\."));
    let Some(value) = value else {
        match std::fs::remove_file(&file) {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => return Err(read_err(e)),
            _ => {}
        }
        return config_unset(git, root, &["--unset-all", "include.path", &pattern]);
    };
    let escaped = value
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n");
    if let Some(dir) = file.parent() {
        std::fs::create_dir_all(dir).map_err(read_err)?;
    }
    std::fs::write(&file, format!("[lfs]\n\tfetchinclude = \"{escaped}\"\n")).map_err(read_err)?;
    git.run(
        root,
        &[
            "config",
            "--local",
            "--replace-all",
            "include.path",
            INCLUDE_CONFIG,
            &pattern,
        ],
        None,
    )
    .map(drop)
}

/// `git config --local <args>` that removes entries; none being there is fine (exit 5).
fn config_unset(git: &GitCli, root: &Path, args: &[&str]) -> Result<(), Error> {
    let mut all = vec!["config", "--local"];
    all.extend_from_slice(args);
    match git.run_status(root, &all, None)? {
        (0 | 5, ..) => Ok(()),
        (_, stdout, stderr) => Err(crate::cli::command_error(
            &all,
            crate::cli::failure_message(&stdout, &stderr),
        )),
    }
}

/// Writes the pointer back over each of `files` whose object is in the store and that
/// has no change, staged or not, then refreshes their index entries (same blob).
fn to_pointers(git: &GitCli, repo: &Repo, files: &[&LfsFile]) -> Result<(), Error> {
    if files.is_empty() {
        return Ok(());
    }
    let changed: BTreeSet<String> = repo.status()?.entries.into_iter().map(|e| e.path).collect();
    let gix = repo.gix();
    let index = gix.index_or_empty().map_err(read_err)?;
    let mut done = Vec::new();
    for f in files {
        if changed.contains(&f.name) {
            continue;
        }
        let Some(entry) = index.entry_by_path(f.name.as_str().into()) else {
            continue;
        };
        let pointer = gix.find_blob(entry.id).map_err(read_err)?.data.clone();
        let Some((oid, _)) = parse_lfs_pointer(&pointer) else {
            continue;
        };
        if !lfs_is_hydrated(repo, &oid) {
            continue;
        }
        std::fs::write(repo.root().join(&f.name), &pointer).map_err(read_err)?;
        done.push(f.name.clone());
    }
    if done.is_empty() {
        return Ok(());
    }
    git.run(
        repo.root(),
        &[
            "--literal-pathspecs",
            "add",
            "--pathspec-from-file=-",
            "--pathspec-file-nul",
        ],
        Some(done.join("\0").as_bytes()),
    )
    .map(drop)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_guids_in_yaml_and_json() {
        let yaml =
            b"m_Script: {fileID: 11500000, guid: 0123456789abcdef0123456789abcdef, type: 3}\n";
        let json = br#"{"m_GUID": "fedcba9876543210fedcba9876543210", "guid":"0000000000000000000000000000000g"}"#;
        assert_eq!(guids(yaml), ["0123456789abcdef0123456789abcdef"]);
        assert_eq!(guids(json), ["fedcba9876543210fedcba9876543210"]);
        assert!(
            readable(b"%YAML 1.1\n")
                && readable(b"  {\"a\":1}")
                && readable(b"fileFormatVersion: 2")
        );
        assert!(!readable(b"\0\0\0\x16UnityFS"));
    }

    #[test]
    fn patterns_never_miss_a_path() {
        assert_eq!(pattern("Assets/a b/c.png"), "Assets/a b/c.png");
        assert_eq!(pattern("Assets/x,y [1]*.png"), "Assets/x?y ?1??.png");
        assert_eq!(pattern("#notes.png"), "?notes.png");
        assert_eq!(pattern("Assets/a#b.png"), "Assets/a#b.png");
        assert_eq!(pattern("trailing "), "trailing?");
    }

    #[test]
    fn whole_folders_become_one_pattern_below_the_top_level() {
        let all: Vec<String> = [
            "Assets/A/1.png",
            "Assets/A/2.png",
            "Assets/A/B/3.png",
            "Assets/C/4.png",
            "Assets/C/5.png",
            "top.png",
        ]
        .map(String::from)
        .to_vec();
        let wanted: BTreeSet<String> = [
            "Assets/A/1.png",
            "Assets/A/2.png",
            "Assets/A/B/3.png",
            "Assets/C/4.png",
            "top.png",
        ]
        .map(String::from)
        .into();
        assert_eq!(
            include_patterns(&wanted, &all),
            ["Assets/A", "Assets/C/4.png", "top.png"]
        );
    }

    #[test]
    fn unity_always_loads_code_plugins_and_special_folders() {
        assert!(always_needed("Packages/x.tgz", ""));
        assert!(always_needed("Assets/Plugins/x64/opencv.dll", ""));
        assert!(always_needed("Assets/UI/Resources/Logo.png", ""));
        assert!(always_needed("Assets/Tools/Editor/Icon.png", ""));
        assert!(!always_needed("Assets/Plugins/Pack/Tex.png", ""));
        assert!(!always_needed("Assets/Resources.png", ""));
        assert!(
            always_needed("Assets/x.png", "Game/"),
            "outside the project"
        );
        assert!(!always_needed("Game/Assets/x.png", "Game/"));
    }

    #[test]
    fn counts_skipped_files_by_folder_up_to_three_levels() {
        assert_eq!(
            folder_of("Assets/Plugins/Pack/Tex/a.png"),
            "Assets/Plugins/Pack"
        );
        assert_eq!(folder_of("Assets/a.png"), "Assets");
        assert_eq!(folder_of("a.png"), "(top level)");
    }
}
