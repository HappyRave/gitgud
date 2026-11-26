use std::sync::Arc;

use super::{MergeCheck, RowPick, conflict_merged};
use crate::image::Version;
use crate::model::cache::Origin;
use crate::repo::{Error, Repo};

impl Repo {
    /// `path`'s unsaved property merge result, when one was drafted.
    pub(crate) fn draft(&self, path: &str) -> Option<Arc<Vec<u8>>> {
        self.drafts
            .lock()
            .expect("drafts lock poisoned")
            .get(path)
            .cloned()
    }

    /// Replaces `path`'s draft (or drops it), and what the 3D previews read from the old one.
    pub(crate) fn set_draft(&self, path: &str, bytes: Option<Vec<u8>>) {
        let mut drafts = self.drafts.lock().expect("drafts lock poisoned");
        match bytes {
            Some(b) => drafts.insert(path.to_owned(), Arc::new(b)),
            None => drafts.remove(path),
        };
        drop(drafts);
        self.model_infos
            .lock()
            .expect("model info lock poisoned")
            .forget(&Origin::Live("draft"));
        let suffix = format!("@{:?}", Version::Draft);
        self.inline_sources
            .lock()
            .expect("inline source lock poisoned")
            .retain(|(k, _)| !k.ends_with(&suffix));
    }
}

/// Keeps conflicted `path` merged with `picks` as its draft, read as [`Version::Draft`].
///
/// # Errors
/// [`Error::Refused`] when it can't be merged by property or a pick is stale.
pub fn draft_merge(repo: &Repo, path: &str, picks: &[RowPick]) -> Result<MergeCheck, Error> {
    let (merged, _) = conflict_merged(repo, path, picks)?;
    repo.set_draft(path, Some(merged.text.into_bytes()));
    Ok(MergeCheck {
        unresolved: merged.unresolved,
        issues: merged.issues,
    })
}
