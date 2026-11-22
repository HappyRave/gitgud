use serde::Serialize;

use crate::cli::GitCli;
use crate::repo::{Error, Repo, read_err};

/// Metadata for a repository tag (lightweight or annotated).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TagInfo {
    pub name: String,
    pub target: String,
    pub short_target: String,
    pub annotated: bool,
    pub message: Option<String>,
    pub time: i64,
}

impl Repo {
    /// Lists tags in the repository, newest first.
    ///
    /// # Errors
    /// [`Error::Read`] when tag references or objects can't be read.
    pub fn tags(&self) -> Result<Vec<TagInfo>, Error> {
        let repo = self.gix();
        let refs = repo.references().map_err(read_err)?;
        let iter = refs.prefixed("refs/tags/").map_err(read_err)?;
        let mut out = Vec::new();

        for item in iter {
            let mut r = item.map_err(|e| Error::Read(e.to_string()))?;
            let full = r.name().as_bstr().to_string();
            let name = full.strip_prefix("refs/tags/").unwrap_or(&full).to_owned();
            let Some(oid) = r.target().try_id().map(ToOwned::to_owned) else {
                continue;
            };
            let mut annotated = false;
            let mut message = None;
            let mut time = 0i64;
            let mut target_id = oid;

            if let Ok(obj) = repo.find_object(oid) {
                if obj.kind == gix::object::Kind::Tag {
                    if let Ok(tag_ref) = obj.try_into_tag() {
                        annotated = true;
                        if let Ok(parsed) = tag_ref.decode() {
                            let msg = parsed.message.to_string();
                            let trimmed = msg.trim();
                            if !trimmed.is_empty() {
                                message = Some(trimmed.to_owned());
                            }
                            if let Ok(Some(tagger)) = parsed.tagger() {
                                time = tagger.seconds();
                            }
                        }
                        if let Ok(peeled) = r.peel_to_commit() {
                            target_id = peeled.id;
                            if time == 0
                                && let Ok(committer) = peeled.committer()
                            {
                                time = committer.seconds();
                            }
                        }
                    }
                } else if let Ok(commit) = obj.try_into_commit() {
                    target_id = commit.id;
                    if let Ok(committer) = commit.committer() {
                        time = committer.seconds();
                    }
                }
            }

            let target = target_id.to_hex().to_string();
            let short_target = target.chars().take(7).collect();
            out.push(TagInfo {
                name,
                target,
                short_target,
                annotated,
                message,
                time,
            });
        }

        out.sort_by(|a, b| b.time.cmp(&a.time).then_with(|| a.name.cmp(&b.name)));
        Ok(out)
    }

    /// Creates a lightweight or annotated tag on `target` (or HEAD when `None`).
    ///
    /// # Errors
    /// [`Error::Refused`] for an empty tag name, or [`Error::Command`] on failure.
    pub fn create_tag(
        &self,
        git: &GitCli,
        name: &str,
        target: Option<&str>,
        message: Option<&str>,
    ) -> Result<(), Error> {
        let name = name.trim();
        if name.is_empty() {
            return Err(Error::Refused("Tag name cannot be empty".into()));
        }
        let msg = message.map(str::trim).filter(|m| !m.is_empty());
        let mut args = vec!["tag"];
        if let Some(m) = msg {
            args.extend(["-a", name, "-m", m]);
        } else {
            args.push(name);
        }
        if let Some(rev) = target.map(str::trim).filter(|r| !r.is_empty()) {
            args.push(rev);
        }
        git.run(self.root(), &args, None).map(drop)
    }

    /// Deletes local tag `name`.
    ///
    /// # Errors
    /// [`Error::Command`] on failure.
    pub fn delete_tag(&self, git: &GitCli, name: &str) -> Result<(), Error> {
        git.run(self.root(), &["tag", "-d", name.trim()], None)
            .map(drop)
    }

    /// Pushes `name` to `remote`.
    ///
    /// # Errors
    /// [`Error::Command`] on failure.
    pub fn push_tag(&self, git: &GitCli, remote: &str, name: &str) -> Result<(), Error> {
        let refspec = format!("refs/tags/{}", name.trim());
        git.run(self.root(), &["push", remote, &refspec], None)
            .map(drop)
    }

    /// Deletes tag `name` from `remote`.
    ///
    /// # Errors
    /// [`Error::Command`] on failure.
    pub fn delete_remote_tag(&self, git: &GitCli, remote: &str, name: &str) -> Result<(), Error> {
        let refspec = format!(":refs/tags/{}", name.trim());
        git.run(self.root(), &["push", remote, &refspec], None)
            .map(drop)
    }
}
