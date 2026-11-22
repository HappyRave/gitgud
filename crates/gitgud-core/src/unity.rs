use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::cli::GitCli;
use crate::library::is_unity_project;
use crate::repo::{EntryKind, Error, Repo, Status, read_err};

/// File extensions handled by UnityYAMLMerge.
pub const UNITY_YAML_EXTENSIONS: [&str; 7] = [
    "unity",
    "prefab",
    "asset",
    "mat",
    "anim",
    "controller",
    "overrideController",
];

const SMART_MERGE_DRIVER: &str = "unityyamlmerge";

/// Unity environment status for an open repository.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UnityInfo {
    pub is_unity: bool,
    pub project_root: Option<String>,
    pub editor_open: bool,
    pub editor_version: Option<String>,
    pub smart_merge_configured: bool,
    pub smart_merge_tool_path: Option<String>,
    pub detected_editors: Vec<String>,
}

/// Classification of a `.meta` consistency warning.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum MetaIssueKind {
    MissingMeta,
    OrphanMeta,
    GuidCollision,
    CaseMismatch,
}

/// Non-blocking warning about `.meta` files paired with assets in Working Copy.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MetaIssue {
    pub kind: MetaIssueKind,
    pub path: String,
    pub detail: String,
}

/// A property changed by both sides or unpredictably resolved during Smart Merge.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VerifyItem {
    pub file_id: String,
    pub object_name: String,
    pub property: String,
    pub base: Option<String>,
    pub ours: Option<String>,
    pub theirs: Option<String>,
    pub merged: Option<String>,
    pub confirmed: bool,
}

/// A property modified on only one side and cleanly kept by Smart Merge.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AutoMergedItem {
    pub file_id: String,
    pub object_name: String,
    pub property: String,
    pub side: String,
    pub value: Option<String>,
}

/// Per-file Smart Merge verification summary kept in `.git/gitgud/merge.json`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UnityMergeFile {
    pub path: String,
    pub verify: Vec<VerifyItem>,
    pub auto_merged: Vec<AutoMergedItem>,
}

/// Finds the Unity project folder inside `repo_root` (either `repo_root` or one level down).
#[must_use]
pub fn find_unity_root(repo_root: &Path) -> Option<PathBuf> {
    if !is_unity_project(repo_root) {
        return None;
    }
    if repo_root
        .join("ProjectSettings/ProjectVersion.txt")
        .is_file()
    {
        return Some(repo_root.to_path_buf());
    }
    let entries = fs::read_dir(repo_root).ok()?;
    for entry in entries.flatten() {
        if entry.file_name() == ".git" {
            continue;
        }
        let p = entry.path();
        if p.is_dir() && p.join("ProjectSettings/ProjectVersion.txt").is_file() {
            return Some(p);
        }
    }
    None
}

/// Detects Unity project metadata, lockfile state, Hub `UnityYAMLMerge` tools, and Smart Merge config.
pub fn detect_unity(git: &GitCli, repo: &Repo) -> Result<UnityInfo, Error> {
    let root = repo.root();
    let Some(project) = find_unity_root(root) else {
        return Ok(UnityInfo {
            is_unity: false,
            project_root: None,
            editor_open: false,
            editor_version: None,
            smart_merge_configured: false,
            smart_merge_tool_path: None,
            detected_editors: Vec::new(),
        });
    };

    let editor_open = project.join("Temp/UnityLockfile").exists();
    let editor_version = read_editor_version(&project);
    let detected_editors = find_yaml_merge_tools(editor_version.as_deref());
    let configured_driver = read_smart_merge_driver(git, root)?;
    let attr_configured = has_unity_merge_attributes(repo);
    let smart_merge_configured = configured_driver.is_some() && attr_configured;

    Ok(UnityInfo {
        is_unity: true,
        project_root: Some(project.to_string_lossy().into_owned()),
        editor_open,
        editor_version,
        smart_merge_configured,
        smart_merge_tool_path: configured_driver.or_else(|| detected_editors.first().cloned()),
        detected_editors,
    })
}

/// Configures Unity Smart Merge in local `.git/config` and `.git/info/attributes` only
/// (never editing tracked `.gitattributes`).
///
/// # Errors
/// [`Error::Refused`] if no `UnityYAMLMerge` binary was found or supplied.
pub fn configure_smart_merge(
    git: &GitCli,
    repo: &Repo,
    explicit_tool_path: Option<&str>,
) -> Result<UnityInfo, Error> {
    let info = detect_unity(git, repo)?;
    let tool = explicit_tool_path
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_owned)
        .or_else(|| info.detected_editors.first().cloned())
        .ok_or_else(|| {
            Error::Refused(
                "UnityYAMLMerge executable was not found in Unity Hub folders; choose its path manually"
                    .into(),
            )
        })?;
    if !Path::new(&tool).is_file() {
        return Err(Error::Refused(format!(
            "UnityYAMLMerge executable does not exist at {tool}"
        )));
    }

    let root = repo.root();
    let normalized = tool.replace('\\', "/");
    let driver = format!("\"{normalized}\" merge -p %O %B %A %A --force --fallback none");
    git.run(
        root,
        &[
            "config",
            "--local",
            "merge.unityyamlmerge.name",
            "Unity SmartMerge",
        ],
        None,
    )?;
    git.run(
        root,
        &["config", "--local", "merge.unityyamlmerge.driver", &driver],
        None,
    )?;
    git.run(
        root,
        &[
            "config",
            "--local",
            "merge.unityyamlmerge.trustExitCode",
            "false",
        ],
        None,
    )?;

    let info_dir = repo.git_dir().join("info");
    fs::create_dir_all(&info_dir).map_err(read_err)?;
    let attr_file = info_dir.join("attributes");
    let existing = fs::read_to_string(&attr_file).unwrap_or_default();
    let mut lines: Vec<String> = existing.lines().map(str::to_owned).collect();
    let mut changed = false;
    for ext in UNITY_YAML_EXTENSIONS {
        let rule = format!("*.{ext} merge={SMART_MERGE_DRIVER}");
        if !lines.iter().any(|l| l.trim() == rule) {
            lines.push(rule);
            changed = true;
        }
    }
    if changed || !attr_file.exists() {
        let mut body = lines.join("\n");
        if !body.is_empty() {
            body.push('\n');
        }
        fs::write(&attr_file, body).map_err(read_err)?;
    }

    detect_unity(git, repo)
}

/// Checks whether a git ref/range rewrites files under `Assets/` (for open-editor warning).
///
/// # Errors
/// [`Error::Command`] when `git diff` fails.
pub fn range_touches_assets(git: &GitCli, root: &Path, range: &str) -> Result<bool, Error> {
    let out = git.run(
        root,
        &["diff", "--name-only", "--no-renames", "-z", range],
        None,
    )?;
    Ok(out
        .split('\0')
        .filter(|p| !p.is_empty())
        .any(|p| p == "Assets" || p.starts_with("Assets/") || p.contains("/Assets/")))
}

/// Expands changed file paths with any changed `.meta` <-> asset twin present in `status`.
#[must_use]
pub fn expand_meta_pairs(status: &Status, paths: &[String]) -> (Vec<String>, Vec<String>) {
    let present: BTreeSet<&str> = status.entries.iter().map(|e| e.path.as_str()).collect();
    let mut out: Vec<String> = Vec::with_capacity(paths.len() * 2);
    let mut seen: BTreeSet<String> = BTreeSet::new();
    let mut paired_notes = Vec::new();

    for p in paths {
        if seen.insert(p.clone()) {
            out.push(p.clone());
        }
        let twin = if let Some(asset) = p.strip_suffix(".meta") {
            asset.to_owned()
        } else {
            format!("{p}.meta")
        };
        if present.contains(twin.as_str()) && seen.insert(twin.clone()) {
            paired_notes.push(twin.clone());
            out.push(twin);
        }
    }
    (out, paired_notes)
}

/// Checks changed paths under `Assets/` for missing `.meta`, orphan `.meta`, GUID collisions, or case mismatches.
#[must_use]
pub fn check_meta_issues(repo: &Repo, status: &Status) -> Vec<MetaIssue> {
    let root = repo.root();
    let Some(project) = find_unity_root(root) else {
        return Vec::new();
    };
    let Ok(proj_prefix) = project.strip_prefix(root) else {
        return Vec::new();
    };
    let assets_prefix = if proj_prefix.as_os_str().is_empty() {
        "Assets/".to_owned()
    } else {
        format!(
            "{}/Assets/",
            proj_prefix.to_string_lossy().replace('\\', "/")
        )
    };

    let mut issues = Vec::new();
    let mut changed_meta_guids: Vec<(String, String)> = Vec::new();
    let mut checked_parents: HashMap<PathBuf, Vec<String>> = HashMap::new();

    for entry in &status.entries {
        let p = &entry.path;
        if !p.starts_with(&assets_prefix) {
            continue;
        }
        let rel = p.strip_prefix(&assets_prefix).unwrap_or("");
        if rel
            .split('/')
            .any(|seg| seg.starts_with('.') || seg.ends_with('~'))
        {
            continue;
        }
        let full = root.join(p);
        if p.ends_with(".meta") {
            if full.is_file() {
                let asset_rel = p.trim_end_matches(".meta");
                if !root.join(asset_rel).exists() {
                    issues.push(MetaIssue {
                        kind: MetaIssueKind::OrphanMeta,
                        path: p.clone(),
                        detail: format!(
                            "Orphan .meta file: matching asset {asset_rel} is missing on disk"
                        ),
                    });
                }
                if let Some(guid) = read_meta_guid(&full) {
                    changed_meta_guids.push((p.clone(), guid));
                }
            }
        } else if entry.kind != EntryKind::Conflicted && full.exists() {
            let meta = root.join(format!("{p}.meta"));
            if !meta.exists() {
                issues.push(MetaIssue {
                    kind: MetaIssueKind::MissingMeta,
                    path: p.clone(),
                    detail: format!("Missing .meta file for {p}; open Unity Editor to generate it"),
                });
            }
            if let Some(parent) = full.parent()
                && let Some(fname) = full.file_name().and_then(|n| n.to_str())
            {
                let siblings = checked_parents
                    .entry(parent.to_path_buf())
                    .or_insert_with(|| {
                        fs::read_dir(parent)
                            .ok()
                            .into_iter()
                            .flatten()
                            .flatten()
                            .filter_map(|e| e.file_name().into_string().ok())
                            .collect()
                    });
                let lower = fname.to_ascii_lowercase();
                if siblings
                    .iter()
                    .any(|s| s.to_ascii_lowercase() == lower && s != fname)
                {
                    issues.push(MetaIssue {
                        kind: MetaIssueKind::CaseMismatch,
                        path: p.clone(),
                        detail: format!("Case-only sibling conflict near {p}"),
                    });
                }
            }
        }
    }

    if !changed_meta_guids.is_empty() {
        let assets_dir = project.join("Assets");
        if assets_dir.is_dir() {
            let mut guid_owners: HashMap<String, Vec<String>> = HashMap::new();
            collect_asset_guids(root, &assets_dir, &mut guid_owners);
            for (meta_path, guid) in changed_meta_guids {
                if let Some(owners) = guid_owners.get(&guid) {
                    let others: Vec<&str> = owners
                        .iter()
                        .map(String::as_str)
                        .filter(|o| !o.eq_ignore_ascii_case(&meta_path))
                        .collect();
                    if !others.is_empty() {
                        issues.push(MetaIssue {
                            kind: MetaIssueKind::GuidCollision,
                            path: meta_path.clone(),
                            detail: format!("GUID {guid} collides with {}", others.join(", ")),
                        });
                    }
                }
            }
        }
    }

    issues
}

/// Runs configured Unity Smart Merge driver against `(base, ours, theirs)` into worktree `path`
/// and compares properties when Smart Merge succeeds.
pub(crate) fn run_smart_merge_on_conflict(
    git: &GitCli,
    repo: &Repo,
    path: &str,
    base: &[u8],
    ours: &[u8],
    theirs: &[u8],
) -> Result<Option<UnityMergeFile>, Error> {
    let Some(driver) = read_smart_merge_driver(git, repo.root())? else {
        return Ok(None);
    };
    let dir = repo.git_dir().join("gitgud").join("smart-merge");
    fs::create_dir_all(&dir).map_err(read_err)?;
    let p_base = dir.join("base");
    let p_ours = dir.join("ours");
    let p_theirs = dir.join("theirs");
    let p_merged = dir.join("merged");
    fs::write(&p_base, base).map_err(read_err)?;
    fs::write(&p_ours, ours).map_err(read_err)?;
    fs::write(&p_theirs, theirs).map_err(read_err)?;
    fs::write(&p_merged, ours).map_err(read_err)?;

    let ok = invoke_merge_driver(&driver, repo.root(), &p_base, &p_merged, &p_theirs)?;
    if !ok {
        return Ok(None);
    }
    let merged_bytes = fs::read(&p_merged).map_err(read_err)?;
    let base_s = String::from_utf8_lossy(base);
    let ours_s = String::from_utf8_lossy(ours);
    let theirs_s = String::from_utf8_lossy(theirs);
    let merged_s = String::from_utf8_lossy(&merged_bytes);
    if merged_s.contains("<<<<<<<") || merged_s.contains(">>>>>>>") {
        return Ok(None);
    }

    fs::write(repo.root().join(path), &merged_bytes).map_err(read_err)?;
    git.run(
        repo.root(),
        &["--literal-pathspecs", "add", "--", path],
        None,
    )?;

    Ok(Some(compare_unity_merge(
        path, &base_s, &ours_s, &theirs_s, &merged_s,
    )))
}

/// Compares `base`, `ours`, `theirs`, and `merged` Unity YAML objects at property granularity
/// for v1.0 Smart Merge verification.
#[must_use]
pub fn compare_unity_merge(
    path: &str,
    base: &str,
    ours: &str,
    theirs: &str,
    merged: &str,
) -> UnityMergeFile {
    let b_docs = parse_unity_docs(base);
    let o_docs = parse_unity_docs(ours);
    let t_docs = parse_unity_docs(theirs);
    let m_docs = parse_unity_docs(merged);

    let mut all_ids: BTreeSet<String> = BTreeSet::new();
    all_ids.extend(b_docs.keys().cloned());
    all_ids.extend(o_docs.keys().cloned());
    all_ids.extend(t_docs.keys().cloned());
    all_ids.extend(m_docs.keys().cloned());

    let mut verify = Vec::new();
    let mut auto_merged = Vec::new();

    for file_id in all_ids {
        let b = b_docs.get(&file_id);
        let o = o_docs.get(&file_id);
        let t = t_docs.get(&file_id);
        let m = m_docs.get(&file_id);

        let object_name = m
            .or(o)
            .or(t)
            .or(b)
            .map(|d| d.display_name.clone())
            .unwrap_or_else(|| format!("&{file_id}"));

        let mut props: BTreeSet<String> = BTreeSet::new();
        for doc in [b, o, t, m].into_iter().flatten() {
            props.extend(doc.props.keys().cloned());
        }

        for prop in props {
            let bv = b.and_then(|d| d.props.get(&prop)).cloned();
            let ov = o.and_then(|d| d.props.get(&prop)).cloned();
            let tv = t.and_then(|d| d.props.get(&prop)).cloned();
            let mv = m.and_then(|d| d.props.get(&prop)).cloned();

            if bv == ov && bv == tv && bv == mv {
                continue;
            }
            if ov != bv && tv == bv && mv == ov {
                auto_merged.push(AutoMergedItem {
                    file_id: file_id.clone(),
                    object_name: object_name.clone(),
                    property: prop,
                    side: "yours".into(),
                    value: mv,
                });
            } else if tv != bv && ov == bv && mv == tv {
                auto_merged.push(AutoMergedItem {
                    file_id: file_id.clone(),
                    object_name: object_name.clone(),
                    property: prop,
                    side: "theirs".into(),
                    value: mv,
                });
            } else {
                verify.push(VerifyItem {
                    file_id: file_id.clone(),
                    object_name: object_name.clone(),
                    property: prop,
                    base: bv,
                    ours: ov,
                    theirs: tv,
                    merged: mv,
                    confirmed: false,
                });
            }
        }
    }

    UnityMergeFile {
        path: path.to_owned(),
        verify,
        auto_merged,
    }
}

#[derive(Debug)]
struct UnityDoc {
    display_name: String,
    props: BTreeMap<String, String>,
}

fn parse_unity_docs(text: &str) -> BTreeMap<String, UnityDoc> {
    let mut docs = BTreeMap::new();
    let mut current_id: Option<String> = None;
    let mut component = String::new();
    let mut name_hint: Option<String> = None;
    let mut props: BTreeMap<String, String> = BTreeMap::new();
    let mut stack: Vec<(usize, String)> = Vec::new();

    let flush = |docs: &mut BTreeMap<String, UnityDoc>,
                 id: Option<String>,
                 comp: &str,
                 name: Option<String>,
                 props: BTreeMap<String, String>| {
        let Some(id) = id else {
            return;
        };
        let display_name = match (comp.is_empty(), name) {
            (false, Some(n)) => format!("{comp} ({n})"),
            (false, None) => format!("{comp} (&{id})"),
            (true, Some(n)) => format!("{n} (&{id})"),
            (true, None) => format!("&{id}"),
        };
        docs.insert(
            id,
            UnityDoc {
                display_name,
                props,
            },
        );
    };

    for raw in text.lines() {
        let line = raw.trim_end_matches('\r');
        if let Some(rest) = line.strip_prefix("--- !u!") {
            flush(
                &mut docs,
                current_id.take(),
                &component,
                name_hint.take(),
                std::mem::take(&mut props),
            );
            component.clear();
            stack.clear();
            let id = rest
                .split('&')
                .nth(1)
                .map(|s| s.split_whitespace().next().unwrap_or(s).to_owned())
                .unwrap_or_default();
            current_id = Some(id);
            continue;
        }
        if current_id.is_none() || line.trim().is_empty() || line.trim_start().starts_with('#') {
            continue;
        }
        let indent = line.len() - line.trim_start().len();
        let trimmed = line.trim();
        if indent == 0 && trimmed.ends_with(':') && !trimmed.contains(' ') {
            component = trimmed.trim_end_matches(':').to_owned();
            stack.clear();
            continue;
        }
        if let Some((k, v)) = trimmed.split_once(':') {
            let key = k.trim_start_matches("- ").trim();
            let val = v.trim();
            while stack.last().is_some_and(|(ind, _)| *ind >= indent) {
                stack.pop();
            }
            if val.is_empty() {
                stack.push((indent, key.to_owned()));
            } else {
                let full_key = if stack.is_empty() {
                    key.to_owned()
                } else {
                    let prefix = stack
                        .iter()
                        .map(|(_, s)| s.as_str())
                        .collect::<Vec<_>>()
                        .join(".");
                    format!("{prefix}.{key}")
                };
                if key == "m_Name" && name_hint.is_none() {
                    name_hint = Some(val.to_owned());
                }
                props.insert(full_key, val.to_owned());
            }
        }
    }
    flush(&mut docs, current_id, &component, name_hint, props);
    docs
}

fn invoke_merge_driver(
    driver_template: &str,
    cwd: &Path,
    base: &Path,
    ours_out: &Path,
    theirs: &Path,
) -> Result<bool, Error> {
    let args = split_shell_tokens(driver_template);
    let Some(exe) = args.first() else {
        return Err(Error::Read("empty merge.unityyamlmerge.driver".into()));
    };
    let mut cmd = std::process::Command::new(exe);
    cmd.current_dir(cwd);
    for tok in &args[1..] {
        let substituted = tok
            .replace("%O", &base.to_string_lossy())
            .replace("%A", &ours_out.to_string_lossy())
            .replace("%B", &theirs.to_string_lossy());
        cmd.arg(substituted);
    }
    crate::cli::no_console_window(&mut cmd);
    let output = cmd.output().map_err(read_err)?;
    Ok(output.status.success())
}

fn split_shell_tokens(s: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut quote: Option<char> = None;
    for ch in s.chars() {
        match quote {
            Some(q) if ch == q => quote = None,
            Some(_) => cur.push(ch),
            None if ch == '"' || ch == '\'' => quote = Some(ch),
            None if ch.is_whitespace() => {
                if !cur.is_empty() {
                    out.push(std::mem::take(&mut cur));
                }
            }
            None => cur.push(ch),
        }
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out
}

fn read_editor_version(project: &Path) -> Option<String> {
    let txt = fs::read_to_string(project.join("ProjectSettings/ProjectVersion.txt")).ok()?;
    for line in txt.lines() {
        if let Some(rest) = line.strip_prefix("m_EditorVersion:") {
            let v = rest.trim();
            if !v.is_empty() {
                return Some(v.to_owned());
            }
        }
    }
    None
}

fn read_smart_merge_driver(git: &GitCli, root: &Path) -> Result<Option<String>, Error> {
    let (code, stdout, _) = git.run_status(
        root,
        &["config", "--get", "merge.unityyamlmerge.driver"],
        None,
    )?;
    if code == 0 {
        let s = stdout.trim();
        Ok((!s.is_empty()).then_some(s.to_owned()))
    } else {
        Ok(None)
    }
}

fn has_unity_merge_attributes(repo: &Repo) -> bool {
    let info_ok = fs::read_to_string(repo.git_dir().join("info/attributes"))
        .map(|s| s.contains("merge=unityyamlmerge"))
        .unwrap_or(false);
    info_ok
        || fs::read_to_string(repo.root().join(".gitattributes"))
            .map(|s| s.contains("merge=unityyamlmerge"))
            .unwrap_or(false)
}

fn find_yaml_merge_tools(preferred: Option<&str>) -> Vec<String> {
    let mut found: Vec<String> = Vec::new();
    if let Ok(env_path) = std::env::var("UNITY_YAML_MERGE") {
        let p = PathBuf::from(env_path.trim());
        if p.is_file() {
            found.push(p.to_string_lossy().into_owned());
        }
    }
    for base in [
        r"C:\Program Files\Unity\Hub\Editor",
        r"C:\Program Files (x86)\Unity\Hub\Editor",
    ] {
        let Ok(entries) = fs::read_dir(base) else {
            continue;
        };
        let mut versions: Vec<(String, PathBuf)> = entries
            .flatten()
            .filter_map(|e| {
                let ver = e.file_name().into_string().ok()?;
                let exe = e.path().join(r"Editor\Data\Tools\UnityYAMLMerge.exe");
                exe.is_file().then_some((ver, exe))
            })
            .collect();
        versions.sort_by(|a, b| {
            let a_pref = preferred == Some(a.0.as_str());
            let b_pref = preferred == Some(b.0.as_str());
            b_pref.cmp(&a_pref).then_with(|| b.0.cmp(&a.0))
        });
        for (_, exe) in versions {
            let s = exe.to_string_lossy().into_owned();
            if !found.contains(&s) {
                found.push(s);
            }
        }
    }
    found
}

pub(crate) fn read_meta_guid(path: &Path) -> Option<String> {
    let content = fs::read_to_string(path).ok()?;
    for line in content.lines().take(12) {
        if let Some(rest) = line.trim().strip_prefix("guid:") {
            let g = rest.trim().to_ascii_lowercase();
            if g.len() == 32 && g.bytes().all(|b| b.is_ascii_hexdigit()) {
                return Some(g);
            }
        }
    }
    None
}

pub(crate) fn collect_asset_guids(
    repo_root: &Path,
    dir: &Path,
    out: &mut HashMap<String, Vec<String>>,
) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let name = entry.file_name();
        let name_str = name.to_string_lossy();
        if name_str.starts_with('.') || name_str.ends_with('~') {
            continue;
        }
        let p = entry.path();
        if p.is_dir() {
            collect_asset_guids(repo_root, &p, out);
        } else if p.extension().and_then(|e| e.to_str()) == Some("meta")
            && let Some(guid) = read_meta_guid(&p)
            && let Ok(rel) = p.strip_prefix(repo_root)
        {
            out.entry(guid)
                .or_default()
                .push(rel.to_string_lossy().replace('\\', "/"));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compare_unity_merge_separates_auto_merged_from_both_sides_verify() {
        let doc = |x: &str, name: &str, speed: &str| {
            format!(
                "--- !u!4 &100\nTransform:\n  m_Name: {name}\n  m_LocalPosition: {{x: {x}, y: 0, z: 0}}\n--- !u!114 &200\nMonoBehaviour:\n  m_Name: Engine\n  speed: {speed}\n"
            )
        };
        let base = doc("0", "Ship", "10");
        let ours = doc("1", "Ship", "20");
        let theirs = doc("0", "PlayerShip", "30");
        let merged = doc("1", "PlayerShip", "30");

        let file =
            compare_unity_merge("Assets/Prefabs/Ship.prefab", &base, &ours, &theirs, &merged);
        assert_eq!(file.auto_merged.len(), 2);
        assert!(
            file.auto_merged
                .iter()
                .any(|a| a.property == "m_LocalPosition" && a.side == "yours")
        );
        assert!(
            file.auto_merged
                .iter()
                .any(|a| a.property == "m_Name" && a.side == "theirs")
        );

        assert_eq!(file.verify.len(), 1);
        assert_eq!(file.verify[0].file_id, "200");
        assert_eq!(file.verify[0].property, "speed");
        assert_eq!(file.verify[0].ours.as_deref(), Some("20"));
        assert_eq!(file.verify[0].theirs.as_deref(), Some("30"));
        assert!(!file.verify[0].confirmed);
    }
}
