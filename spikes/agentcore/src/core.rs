use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

pub type Result<T> = std::result::Result<T, String>;

#[derive(Debug, Clone, serde::Serialize, schemars::JsonSchema)]
pub struct Hunk {
    pub id: String,
    pub path: String,
    pub header: String,
    pub added: usize,
    pub removed: usize,
    #[serde(skip)]
    file_header: String,
    #[serde(skip)]
    body: String,
}

#[derive(Debug, Clone, serde::Serialize, schemars::JsonSchema)]
pub struct Proposal {
    pub id: String,
    pub commit: String,
    pub message: String,
    pub files: Vec<String>,
    pub flags: Vec<String>,
}

pub struct Repo {
    root: PathBuf,
}

impl Repo {
    pub fn open(root: impl AsRef<Path>) -> Self {
        Self { root: root.as_ref().to_path_buf() }
    }

    fn git(&self, args: &[&str], env: &[(&str, &str)], stdin: Option<&str>) -> Result<String> {
        let mut cmd = Command::new("git");
        cmd.args(args).current_dir(&self.root).env("GIT_TERMINAL_PROMPT", "0").env_remove("GIT_CURL_VERBOSE");
        for (k, v) in env {
            cmd.env(k, v);
        }
        cmd.stdin(if stdin.is_some() { Stdio::piped() } else { Stdio::null() }).stdout(Stdio::piped()).stderr(Stdio::piped());
        let mut child = cmd.spawn().map_err(|e| e.to_string())?;
        if let Some(input) = stdin {
            child.stdin.take().ok_or("no stdin")?.write_all(input.as_bytes()).map_err(|e| e.to_string())?;
        }
        let out = child.wait_with_output().map_err(|e| e.to_string())?;
        if !out.status.success() {
            return Err(format!("git {}: {}", args.join(" "), String::from_utf8_lossy(&out.stderr).trim()));
        }
        Ok(String::from_utf8_lossy(&out.stdout).into_owned())
    }

    pub fn hunks(&self) -> Result<Vec<Hunk>> {
        let diff = self.git(&["diff", "--no-color", "--no-ext-diff", "-U3"], &[], None)?;
        let mut out = Vec::new();
        let (mut file_header, mut path) = (String::new(), String::new());
        let mut cur: Option<(String, String)> = None;
        let flush = |cur: &mut Option<(String, String)>, file_header: &str, path: &str, out: &mut Vec<Hunk>| {
            if let Some((header, body)) = cur.take() {
                let added = body.lines().filter(|l| l.starts_with('+')).count();
                let removed = body.lines().filter(|l| l.starts_with('-')).count();
                // Line numbers are left out of the ID so it survives edits elsewhere in the file.
                let id = format!("{:08x}", fnv(&format!("{path}\n{body}")) as u32);
                out.push(Hunk { id, path: path.to_string(), header, added, removed, file_header: file_header.to_string(), body });
            }
        };
        for line in diff.lines() {
            if line.starts_with("diff --git ") {
                flush(&mut cur, &file_header, &path, &mut out);
                file_header = format!("{line}\n");
                path = line.rsplit(" b/").next().unwrap_or("").to_string();
            } else if line.starts_with("@@") {
                flush(&mut cur, &file_header, &path, &mut out);
                cur = Some((line.to_string(), String::new()));
            } else if let Some((_, body)) = cur.as_mut() {
                body.push_str(line);
                body.push('\n');
            } else {
                file_header.push_str(line);
                file_header.push('\n');
            }
        }
        flush(&mut cur, &file_header, &path, &mut out);
        Ok(out)
    }

    pub fn propose(&self, ids: &[String], message: &str, why: &str) -> Result<Proposal> {
        let all = self.hunks()?;
        let chosen: Vec<&Hunk> = all.iter().filter(|h| ids.contains(&h.id)).collect();
        let missing: Vec<&String> = ids.iter().filter(|id| !all.iter().any(|h| &h.id == *id)).collect();
        if !missing.is_empty() {
            return Err(format!("unknown or stale hunk ids: {missing:?} (call status again)"));
        }
        let mut patch = String::new();
        let mut last_file = "";
        for h in &chosen {
            if h.file_header != last_file {
                patch.push_str(&h.file_header);
                last_file = &h.file_header;
            }
            patch.push_str(&h.header);
            patch.push('\n');
            patch.push_str(&h.body);
        }
        let head = self.git(&["rev-parse", "HEAD"], &[], None)?.trim().to_string();
        let tmp = std::env::temp_dir().join(format!("gitgud-proposal-{}.index", std::process::id()));
        let tmp_s = tmp.to_string_lossy().to_string();
        let env = [("GIT_INDEX_FILE", tmp_s.as_str())];
        self.git(&["read-tree", "HEAD"], &env, None)?;
        self.git(&["apply", "--cached", "--recount", "-"], &env, Some(&patch))?;
        let tree = self.git(&["write-tree"], &env, None)?.trim().to_string();
        let _ = std::fs::remove_file(&tmp);
        let full = format!("{message}\n\nGitgud-Why: {why}\nGitgud-Base: {head}\n");
        let commit = self.git(&["commit-tree", &tree, "-p", &head, "-F", "-"], &[], Some(&full))?.trim().to_string();
        let id = commit[..8].to_string();
        self.git(&["update-ref", &format!("refs/gitgud/proposals/{id}"), &commit], &[], None)?;
        self.describe(&id, &commit)
    }

    fn describe(&self, id: &str, commit: &str) -> Result<Proposal> {
        let message = self.git(&["log", "-1", "--format=%B", commit], &[], None)?;
        let files: Vec<String> = self.git(&["diff", "--name-only", &format!("{commit}^"), commit], &[], None)?.lines().map(String::from).collect();
        Ok(Proposal { id: id.to_string(), commit: commit.to_string(), message: message.trim().to_string(), flags: self.flags(&files)?, files })
    }

    fn flags(&self, files: &[String]) -> Result<Vec<String>> {
        let mut flags = Vec::new();
        for f in files {
            let attr = self.git(&["check-attr", "filter", "lockable", "--", f], &[], None)?;
            if attr.contains("filter: lfs") {
                flags.push(format!("LFS file: {f}"));
            }
            if attr.contains("lockable: set") {
                flags.push(format!("lockable file, check lock owner: {f}"));
            }
            let is_asset = f.starts_with("Assets/") && !f.ends_with(".meta");
            if is_asset && !files.contains(&format!("{f}.meta")) && !self.root.join(format!("{f}.meta")).exists() {
                flags.push(format!("missing .meta for {f}"));
            }
        }
        Ok(flags)
    }

    pub fn list(&self) -> Result<Vec<Proposal>> {
        let refs = self.git(&["for-each-ref", "--format=%(refname:lstrip=3) %(objectname)", "refs/gitgud/proposals/"], &[], None)?;
        refs.lines().filter_map(|l| l.split_once(' ')).map(|(id, c)| self.describe(id, c)).collect()
    }

    // Human-only: stages exactly the proposal and commits through `git commit`, so hooks and signing run.
    pub fn approve(&self, id: &str) -> Result<String> {
        let refname = format!("refs/gitgud/proposals/{id}");
        let commit = self.git(&["rev-parse", &refname], &[], None)?.trim().to_string();
        let head = self.git(&["rev-parse", "HEAD"], &[], None)?.trim().to_string();
        let base = self.git(&["rev-parse", &format!("{commit}^")], &[], None)?.trim().to_string();
        if base != head {
            return Err(format!("HEAD moved since the proposal ({base} -> {head}); replay needed"));
        }
        if !self.git(&["diff", "--cached", "--name-only"], &[], None)?.trim().is_empty() {
            return Err("your index has staged changes; commit or unstage them first".into());
        }
        let patch = self.git(&["diff", "--binary", &head, &commit], &[], None)?;
        self.git(&["apply", "--cached", "-"], &[], Some(&patch))?;
        let msg = self.git(&["log", "-1", "--format=%B", &commit], &[], None)?;
        self.git(&["commit", "-q", "-F", "-"], &[], Some(&msg))?;
        self.git(&["update-ref", "-d", &refname], &[], None)?;
        Ok(self.git(&["rev-parse", "HEAD"], &[], None)?.trim().to_string())
    }
}

fn fnv(s: &str) -> u64 {
    s.bytes().fold(0xcbf29ce484222325, |h, b| (h ^ b as u64).wrapping_mul(0x100000001b3))
}
