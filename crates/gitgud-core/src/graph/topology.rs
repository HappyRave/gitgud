use std::collections::HashMap;

use gix::ObjectId;
use gix::traverse::commit::topo;
use serde::Serialize;

use super::Stroke;
use crate::repo::{Error, Repo, read_err};

/// A ref pointing at a commit, as drawn on its graph row.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum RefLabel {
    /// HEAD when detached.
    Head,
    Branch {
        name: String,
        current: bool,
    },
    Remote {
        name: String,
    },
    Tag {
        name: String,
    },
}

/// Every commit reachable from branches, remote branches, tags and HEAD, in
/// `git rev-list --topo-order` order, with parents as row numbers. Optionally
/// starts with a virtual "uncommitted changes" row whose parent is HEAD.
pub struct Topology {
    ids: Vec<ObjectId>,
    parent_start: Vec<u32>,
    parents: Vec<u32>,
    labels: HashMap<u32, Vec<RefLabel>>,
    wip: bool,
    asked_wip: bool,
    refs: RefSnapshot,
    /// Rows drawn other than solid: a history plan's preview.
    strokes: HashMap<u32, Stroke>,
    /// Planned commits of a preview, which exist in no repository.
    ghosts: HashMap<u32, Ghost>,
}

/// A commit a history plan would create, drawn as a dashed "after" row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ghost {
    pub subject: String,
    /// What the plan does to it: "reworded", "+ fixup folded in"...
    pub tag: String,
}

/// Every drawn ref and where it points, sorted: equal snapshots mean equal history.
type RefSnapshot = Vec<(ObjectId, RefLabel)>;

fn snapshot(labels_by_id: &HashMap<ObjectId, Vec<RefLabel>>) -> RefSnapshot {
    let mut refs: RefSnapshot = labels_by_id
        .iter()
        .flat_map(|(id, labels)| labels.iter().map(|l| (*id, l.clone())))
        .collect();
    refs.sort();
    refs
}

impl Topology {
    /// Whether walking `repo` with `wip` now would give this same topology: no
    /// ref, tag or HEAD has moved, and the WIP row is still wanted (or not).
    ///
    /// # Errors
    /// [`Error::Read`] when refs can't be read.
    pub fn is_current(&self, repo: &Repo, wip: bool) -> Result<bool, Error> {
        if wip != self.asked_wip {
            return Ok(false);
        }
        let (_, labels_by_id) = tips(&repo.gix())?;
        Ok(snapshot(&labels_by_id) == self.refs)
    }

    /// Walks the whole history of `repo`, using its commit-graph file if present.
    /// With `wip`, row 0 is the working copy, on top of HEAD; it is left out when
    /// HEAD has no commit yet.
    ///
    /// # Errors
    /// [`Error::Read`] when refs or commits can't be read.
    pub fn walk(repo: &Repo, wip: bool) -> Result<Self, Error> {
        let repo = repo.gix();
        let (tips, mut labels_by_id) = tips(&repo)?;
        let wip_parent = if wip {
            repo.head_id().ok().map(gix::Id::detach)
        } else {
            None
        };

        let walk = topo::Builder::from_iters(&repo.objects, tips, None::<Vec<ObjectId>>)
            .sorting(topo::Sorting::TopoOrder)
            .with_commit_graph(repo.commit_graph_if_enabled().map_err(read_err)?)
            .build()
            .map_err(read_err)?;

        let mut commits = Vec::new();
        for info in walk {
            let info = info.map_err(read_err)?;
            commits.push((info.id, info.parent_ids));
        }
        let offset = usize::from(wip_parent.is_some());
        let row_of: HashMap<ObjectId, u32> = commits
            .iter()
            .enumerate()
            .map(|(row, (id, _))| (*id, to_u32(row + offset)))
            .collect();

        let rows = commits.len() + offset;
        let mut topology = Self {
            ids: Vec::with_capacity(rows),
            parent_start: Vec::with_capacity(rows + 1),
            parents: Vec::with_capacity(rows + rows / 8),
            labels: HashMap::new(),
            wip: wip_parent.is_some(),
            asked_wip: wip,
            refs: snapshot(&labels_by_id),
            strokes: HashMap::new(),
            ghosts: HashMap::new(),
        };
        if let Some(head) = wip_parent {
            topology.parent_start.push(0);
            topology.parents.push(row_of[&head]);
            topology.ids.push(ObjectId::null(repo.object_hash()));
        }
        for (i, (id, parent_ids)) in commits.into_iter().enumerate() {
            let row = i + offset;
            topology.parent_start.push(to_u32(topology.parents.len()));
            // A parent missing from the walk is cut off by a shallow clone.
            topology
                .parents
                .extend(parent_ids.iter().filter_map(|p| row_of.get(p).copied()));
            if let Some(mut labels) = labels_by_id.remove(&id) {
                labels.sort_by_key(label_order);
                topology.labels.insert(to_u32(row), labels);
            }
            topology.ids.push(id);
        }
        topology.parent_start.push(to_u32(topology.parents.len()));
        Ok(topology)
    }

    pub fn len(&self) -> usize {
        self.ids.len()
    }

    pub fn is_empty(&self) -> bool {
        self.ids.is_empty()
    }

    /// The commit id; a null id for the working-copy row.
    pub fn id(&self, row: usize) -> ObjectId {
        self.ids[row]
    }

    /// Whether `row` is the virtual working-copy row.
    pub fn is_wip(&self, row: usize) -> bool {
        self.wip && row == 0
    }

    /// Parent rows, first parent first. Always greater than `row`.
    pub fn parents(&self, row: usize) -> &[u32] {
        let start = self.parent_start[row] as usize;
        let end = self.parent_start[row + 1] as usize;
        &self.parents[start..end]
    }

    pub fn labels(&self, row: usize) -> &[RefLabel] {
        self.labels.get(&to_u32(row)).map_or(&[], Vec::as_slice)
    }

    pub fn stroke(&self, row: usize) -> Stroke {
        self.strokes.get(&to_u32(row)).copied().unwrap_or_default()
    }

    #[cfg(test)]
    pub(crate) fn set_stroke(&mut self, row: usize, stroke: Stroke) {
        self.strokes.insert(to_u32(row), stroke);
    }

    /// The planned commit drawn on `row`, if it is one.
    pub fn ghost(&self, row: usize) -> Option<&Ghost> {
        self.ghosts.get(&to_u32(row))
    }

    /// The row of commit `id`, if it is in the graph.
    pub fn row_of(&self, id: ObjectId) -> Option<usize> {
        let first = usize::from(self.wip);
        self.ids[first..]
            .iter()
            .position(|i| *i == id)
            .map(|r| r + first)
    }

    /// A copy with `ghosts` (newest first) inserted as a chain before row `at`,
    /// the oldest on top of row `base` (or a root when `None`), and the rows of
    /// `faded` drawn faded: the preview of a history plan.
    ///
    /// # Panics
    /// When `base` is above `at`, or `at` is above the working-copy row.
    pub fn with_ghosts(
        &self,
        at: usize,
        ghosts: Vec<Ghost>,
        base: Option<usize>,
        faded: &[usize],
    ) -> Self {
        assert!(at >= usize::from(self.wip) && at <= self.len());
        assert!(
            base.is_none_or(|b| b >= at),
            "ghosts must sit above their base"
        );
        let k = ghosts.len();
        let moved = |r: usize| if r < at { r } else { r + k };
        let mut t = Self {
            ids: Vec::with_capacity(self.len() + k),
            parent_start: Vec::with_capacity(self.len() + k + 1),
            parents: Vec::with_capacity(self.parents.len() + k),
            labels: self
                .labels
                .iter()
                .map(|(r, l)| (to_u32(moved(*r as usize)), l.clone()))
                .collect(),
            wip: self.wip,
            asked_wip: self.asked_wip,
            refs: self.refs.clone(),
            strokes: HashMap::new(),
            ghosts: HashMap::new(),
        };
        let null = ObjectId::null(self.ids.first().map_or(gix::hash::Kind::Sha1, |i| i.kind()));
        let push_old = |t: &mut Self, row: usize| {
            t.parent_start.push(to_u32(t.parents.len()));
            t.parents
                .extend(self.parents(row).iter().map(|&p| to_u32(moved(p as usize))));
            t.ids.push(self.ids[row]);
        };
        for row in 0..at {
            push_old(&mut t, row);
        }
        for (i, ghost) in ghosts.into_iter().enumerate() {
            let row = at + i;
            t.parent_start.push(to_u32(t.parents.len()));
            if i + 1 < k {
                t.parents.push(to_u32(row + 1));
            } else if let Some(b) = base {
                t.parents.push(to_u32(moved(b)));
            }
            t.ids.push(null);
            t.ghosts.insert(to_u32(row), ghost);
            t.strokes.insert(to_u32(row), Stroke::Dashed);
        }
        for row in at..self.len() {
            push_old(&mut t, row);
        }
        t.parent_start.push(to_u32(t.parents.len()));
        for &row in faded {
            t.strokes.insert(to_u32(moved(row)), Stroke::Faded);
        }
        t
    }

    /// A topology from explicit parent rows, for layout tests.
    #[cfg(any(test, feature = "test-util"))]
    pub fn from_parents(parents: &[&[u32]]) -> Self {
        let mut t = Self {
            ids: vec![ObjectId::null(gix::hash::Kind::Sha1); parents.len()],
            parent_start: Vec::new(),
            parents: Vec::new(),
            labels: HashMap::new(),
            wip: false,
            asked_wip: false,
            refs: Vec::new(),
            strokes: HashMap::new(),
            ghosts: HashMap::new(),
        };
        for (row, ps) in parents.iter().enumerate() {
            assert!(
                ps.iter()
                    .all(|&p| p as usize > row && (p as usize) < parents.len()),
                "row {row}: parents must come later in the list"
            );
            t.parent_start.push(to_u32(t.parents.len()));
            t.parents.extend_from_slice(ps);
        }
        t.parent_start.push(to_u32(t.parents.len()));
        t
    }
}

fn to_u32(n: usize) -> u32 {
    u32::try_from(n).expect("more than u32::MAX commits")
}

fn label_order(label: &RefLabel) -> (u8, String) {
    match label {
        RefLabel::Head => (0, String::new()),
        RefLabel::Branch {
            current: true,
            name,
        } => (0, name.clone()),
        RefLabel::Branch { name, .. } => (1, name.clone()),
        RefLabel::Remote { name } => (2, name.clone()),
        RefLabel::Tag { name } => (3, name.clone()),
    }
}

type Tips = (Vec<ObjectId>, HashMap<ObjectId, Vec<RefLabel>>);

fn tips(repo: &gix::Repository) -> Result<Tips, Error> {
    let mut tips = Vec::new();
    let mut labels: HashMap<ObjectId, Vec<RefLabel>> = HashMap::new();

    let head = repo.head().map_err(read_err)?;
    let current = match &head.kind {
        gix::head::Kind::Symbolic(r) => Some(r.name.clone()),
        gix::head::Kind::Detached { target, .. } => {
            tips.push(*target);
            labels.entry(*target).or_default().push(RefLabel::Head);
            None
        }
        gix::head::Kind::Unborn(_) => None,
    };

    let platform = repo.references().map_err(read_err)?;
    for reference in platform.all().map_err(read_err)? {
        let mut reference = reference.map_err(|e| Error::Read(e.to_string()))?;
        if matches!(reference.target(), gix::refs::TargetRef::Symbolic(_)) {
            continue;
        }
        let full = reference.name().as_bstr().to_string();
        let short = reference.name().shorten().to_string();
        let label = if full.starts_with("refs/heads/") {
            RefLabel::Branch {
                current: current
                    .as_ref()
                    .is_some_and(|c| c.as_ref() == reference.name()),
                name: short,
            }
        } else if full.starts_with("refs/remotes/") {
            RefLabel::Remote { name: short }
        } else if full.starts_with("refs/tags/") {
            RefLabel::Tag { name: short }
        } else {
            continue;
        };
        // Tags may point at trees or blobs; only commits are drawn.
        let Ok(commit) = reference.peel_to_kind(gix::object::Kind::Commit) else {
            continue;
        };
        tips.push(commit.id);
        labels.entry(commit.id).or_default().push(label);
    }
    Ok((tips, labels))
}
