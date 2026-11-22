use std::collections::HashMap;

use serde::Serialize;

use crate::cli::GitCli;
use crate::repo::{Error, Repo, read_err};

/// One commit in a file's history (`git log --follow`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FileCommit {
    pub id: String,
    pub short_id: String,
    pub subject: String,
    pub author: String,
    pub time: i64,
    pub change: String,
}

/// One annotated line returned by `git blame --porcelain`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BlameLine {
    pub line_no: u32,
    pub commit: String,
    pub short_commit: String,
    pub author: String,
    pub time: i64,
    pub summary: String,
    pub text: String,
}

impl Repo {
    /// Lists recent commits touching `path`, following renames (`--follow`).
    ///
    /// # Errors
    /// [`Error::Command`] if `git log` fails.
    pub fn file_history(
        &self,
        git: &GitCli,
        path: &str,
        limit: usize,
    ) -> Result<Vec<FileCommit>, Error> {
        let repo = self.gix();
        if repo.head().map_err(read_err)?.is_unborn() {
            return Ok(Vec::new());
        }
        let n = limit.clamp(1, 500).to_string();
        let out = git.run(
            self.root(),
            &[
                "log",
                "--follow",
                "--name-status",
                "--date=unix",
                "--format=%x1e%H%x1f%h%x1f%s%x1f%an%x1f%at",
                "-n",
                &n,
                "--",
                path,
            ],
            None,
        )?;
        Ok(parse_file_history(&out))
    }

    /// Returns line-by-line blame annotations for `path` at `rev` (or worktree HEAD if omitted).
    ///
    /// # Errors
    /// [`Error::Command`] if `git blame` fails.
    pub fn blame_file(
        &self,
        git: &GitCli,
        path: &str,
        rev: Option<&str>,
    ) -> Result<Vec<BlameLine>, Error> {
        let mut args = vec!["blame", "--porcelain"];
        if let Some(r) = rev.map(str::trim).filter(|r| !r.is_empty()) {
            args.push(r);
        }
        args.extend(["--", path]);
        let (code, stdout, stderr) = git.run_status(self.root(), &args, None)?;
        if code != 0 {
            let msg = crate::cli::failure_message(&stdout, &stderr);
            if msg.contains("no such path") && rev.is_none() {
                return Ok(Vec::new());
            }
            return Err(crate::cli::command_error(&args, msg));
        }
        Ok(parse_blame_porcelain(&stdout))
    }
}

fn parse_file_history(raw: &str) -> Vec<FileCommit> {
    let mut out = Vec::new();
    for rec in raw.split('\x1e') {
        let trimmed = rec.trim();
        if trimmed.is_empty() {
            continue;
        }
        let mut lines = trimmed.lines();
        let Some(header) = lines.next() else {
            continue;
        };
        let cols: Vec<&str> = header.split('\x1f').collect();
        if cols.len() < 5 {
            continue;
        }
        let mut change = "modified".to_owned();
        for status_line in lines {
            let s = status_line.trim();
            if s.is_empty() {
                continue;
            }
            let code = s.split('\t').next().unwrap_or("");
            change = match code.chars().next() {
                Some('A') => "added",
                Some('D') => "deleted",
                Some('R') => "renamed",
                Some('T') => "typeChange",
                _ => "modified",
            }
            .to_owned();
            break;
        }
        out.push(FileCommit {
            id: cols[0].to_owned(),
            short_id: cols[1].to_owned(),
            subject: cols[2].to_owned(),
            author: cols[3].to_owned(),
            time: cols[4].trim().parse().unwrap_or(0),
            change,
        });
    }
    out
}

#[derive(Default, Clone)]
struct BlameMeta {
    author: String,
    time: i64,
    summary: String,
}

fn parse_blame_porcelain(raw: &str) -> Vec<BlameLine> {
    let mut meta: HashMap<String, BlameMeta> = HashMap::new();
    let mut out = Vec::new();
    let mut lines = raw.lines();

    while let Some(header) = lines.next() {
        let mut parts = header.split_whitespace();
        let Some(commit) = parts.next() else {
            continue;
        };
        if commit.len() < 40 || !commit.bytes().all(|b| b.is_ascii_hexdigit()) {
            continue;
        }
        let _orig_line = parts.next();
        let line_no: u32 = parts.next().and_then(|s| s.parse().ok()).unwrap_or(0);
        let entry = meta.entry(commit.to_owned()).or_default();

        let mut text = String::new();
        for l in lines.by_ref() {
            if let Some(code_line) = l.strip_prefix('\t') {
                text = code_line.to_owned();
                break;
            }
            if let Some(a) = l.strip_prefix("author ") {
                entry.author = a.to_owned();
            } else if let Some(t) = l.strip_prefix("author-time ") {
                entry.time = t.trim().parse().unwrap_or(0);
            } else if let Some(s) = l.strip_prefix("summary ") {
                entry.summary = s.to_owned();
            }
        }
        let m = entry.clone();
        out.push(BlameLine {
            line_no,
            short_commit: commit.chars().take(7).collect(),
            commit: commit.to_owned(),
            author: m.author,
            time: m.time,
            summary: m.summary,
            text,
        });
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_git_blame_porcelain_with_cached_headers() {
        let sha1 = "a".repeat(40);
        let sha2 = "b".repeat(40);
        let raw = format!(
            "{sha1} 1 1 2\nauthor Ada\nauthor-time 1700000000\nsummary init\nfilename a.rs\n\tfn main() {{\n\
             {sha1} 2 2\n\t    println!(\"hi\");\n\
             {sha2} 3 3 1\nauthor Grace\nauthor-time 1700000500\nsummary close\nfilename a.rs\n\t}}\n"
        );
        let parsed = parse_blame_porcelain(&raw);
        assert_eq!(parsed.len(), 3);
        assert_eq!(parsed[0].author, "Ada");
        assert_eq!(parsed[1].author, "Ada");
        assert_eq!(parsed[1].text, "    println!(\"hi\");");
        assert_eq!(parsed[2].author, "Grace");
        assert_eq!(parsed[2].summary, "close");
    }
}
