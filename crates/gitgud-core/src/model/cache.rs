use std::sync::Arc;

use super::ModelInfo;

/// How many `model_info` results a repo keeps.
const KEPT_INFOS: usize = 64;

/// Where a version's files come from: a commit's tree never changes, the rest can.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Origin {
    Tree(gix::ObjectId),
    Live(&'static str),
}

/// Recent `model_info` results, least recently used first.
#[derive(Default)]
pub(crate) struct InfoCache {
    entries: Vec<(String, Origin, Arc<ModelInfo>)>,
    /// Bumped whenever live results are dropped, so a result read meanwhile isn't kept.
    generation: u64,
}

impl InfoCache {
    pub fn generation(&self) -> u64 {
        self.generation
    }

    pub fn get(&mut self, path: &str, origin: &Origin) -> Option<Arc<ModelInfo>> {
        let at = self
            .entries
            .iter()
            .position(|(p, o, _)| p == path && o == origin)?;
        let entry = self.entries.remove(at);
        let info = entry.2.clone();
        self.entries.push(entry);
        Some(info)
    }

    /// Keeps `info`, unless live results were dropped since `generation` was read.
    pub fn put(&mut self, path: &str, origin: Origin, info: Arc<ModelInfo>, generation: u64) {
        if matches!(origin, Origin::Live(_)) && generation != self.generation {
            return;
        }
        self.entries
            .retain(|(p, o, _)| !(p == path && *o == origin));
        if self.entries.len() >= KEPT_INFOS {
            self.entries.remove(0);
        }
        self.entries.push((path.to_owned(), origin, info));
    }

    /// Drops the results read from the index or work tree, which may have changed.
    pub fn forget_live(&mut self) {
        self.generation += 1;
        self.entries
            .retain(|(_, o, _)| matches!(o, Origin::Tree(_)));
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::ModelKind;
    use std::collections::BTreeMap;

    fn info(scale: f32) -> Arc<ModelInfo> {
        Arc::new(ModelInfo {
            kind: ModelKind::Fbx,
            key: None,
            materials: Vec::new(),
            remaps: BTreeMap::new(),
            scale,
            scene: None,
        })
    }

    #[test]
    fn keeps_recent_results_and_drops_live_ones_on_request() {
        let tree = Origin::Tree(gix::ObjectId::empty_tree(gix::hash::Kind::Sha1));
        let live = Origin::Live("worktree");
        let mut cache = InfoCache::default();
        let g = cache.generation();
        cache.put("a.fbx", tree.clone(), info(1.0), g);
        cache.put("a.fbx", live.clone(), info(2.0), g);
        assert_eq!(cache.get("a.fbx", &tree).unwrap().scale, 1.0);
        assert_eq!(cache.get("a.fbx", &live).unwrap().scale, 2.0);
        assert!(cache.get("b.fbx", &tree).is_none());

        cache.forget_live();
        assert!(cache.get("a.fbx", &live).is_none());
        assert_eq!(cache.len(), 1);
        cache.put("a.fbx", live.clone(), info(3.0), g);
        assert!(
            cache.get("a.fbx", &live).is_none(),
            "read before the files changed, so not kept"
        );

        let g = cache.generation();
        for i in 0..KEPT_INFOS {
            if i == KEPT_INFOS / 2 {
                cache.get("a.fbx", &tree);
            }
            cache.put(&format!("{i}.fbx"), live.clone(), info(0.0), g);
        }
        assert_eq!(cache.len(), KEPT_INFOS);
        assert!(
            cache.get("a.fbx", &tree).is_some(),
            "used recently, so kept"
        );
        assert!(cache.get("0.fbx", &live).is_none(), "the oldest went first");
    }
}
