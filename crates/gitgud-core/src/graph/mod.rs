mod layout;
mod topology;

pub use layout::{Layout, MAX_LANES, Part, RowLayout, Segment, Stroke};
pub use topology::{Ghost, RefLabel, Topology};

use serde::Serialize;

use crate::repo::{Error, Repo, read_err};

/// A repository's commit graph: topology and lane checkpoints, built once.
pub struct Graph {
    topology: Topology,
    layout: Layout,
}

/// One graph row, ready to draw.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Row {
    /// Full hex id; empty for the working-copy row.
    pub id: String,
    pub short_id: String,
    pub subject: String,
    pub author: String,
    /// Author time, seconds since the Unix epoch.
    pub time: i64,
    pub labels: Vec<RefLabel>,
    pub wip: bool,
    /// On a plan preview's planned commit: what the plan does to it.
    pub after: Option<String>,
    /// On a plan preview: a commit the plan replaces.
    pub faded: bool,
    /// Stash index (`stash@{n}`) when this row represents a stash.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stash: Option<usize>,
    /// Agent proposal ID when this row represents an active proposal.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub proposal: Option<String>,
    pub layout: RowLayout,
}

impl Graph {
    /// Walks `repo` and lays out lanes. `wip` adds the working-copy row on top.
    ///
    /// # Errors
    /// [`Error::Read`] when the history can't be read.
    pub fn build(repo: &Repo, wip: bool) -> Result<Self, Error> {
        let topology = Topology::walk(repo, wip)?;
        let layout = Layout::build(&topology);
        Ok(Self { topology, layout })
    }

    /// This graph with a history plan drawn in: `ghosts` (newest first) chained
    /// on top of commit `base`, above whichever of `base` and `tip` comes first,
    /// and the commits in `replaced` faded.
    ///
    /// # Errors
    /// [`Error::Refused`] when `tip` or `base` isn't in the graph.
    pub fn preview(
        &self,
        tip: gix::ObjectId,
        base: gix::ObjectId,
        ghosts: Vec<Ghost>,
        replaced: &[gix::ObjectId],
    ) -> Result<Self, Error> {
        let missing = |id: gix::ObjectId| Error::Refused(format!("{id} isn't in the graph"));
        let tip_row = self.topology.row_of(tip).ok_or_else(|| missing(tip))?;
        let base_row = self.topology.row_of(base).ok_or_else(|| missing(base))?;
        let faded: Vec<usize> = replaced
            .iter()
            .filter_map(|id| self.topology.row_of(*id))
            .collect();
        let topology =
            self.topology
                .with_ghosts(tip_row.min(base_row), ghosts, Some(base_row), &faded);
        let layout = Layout::build(&topology);
        Ok(Self { topology, layout })
    }

    /// Whether [`Graph::build`] with `wip` would give this same graph now.
    ///
    /// # Errors
    /// [`Error::Read`] when refs can't be read.
    pub fn is_current(&self, repo: &Repo, wip: bool) -> Result<bool, Error> {
        self.topology.is_current(repo, wip)
    }

    /// Number of rows, the working-copy row included.
    pub fn len(&self) -> usize {
        self.topology.len()
    }

    pub fn is_empty(&self) -> bool {
        self.topology.is_empty()
    }

    /// Rows whose subject, author or ref labels contain `query` (case-insensitive),
    /// or whose id starts with it, in graph order. The working-copy row never matches.
    ///
    /// # Errors
    /// [`Error::Read`] when a commit can't be read.
    pub fn find(&self, repo: &Repo, query: &str) -> Result<Vec<usize>, Error> {
        let needle = query.trim().to_lowercase();
        if needle.is_empty() {
            return Ok(Vec::new());
        }
        let repo = repo.gix();
        let mut hits = Vec::new();
        for row in 0..self.len() {
            if self.topology.is_wip(row) || self.topology.ghost(row).is_some() {
                continue;
            }
            let id = self.topology.id(row);
            let label_hit = self.topology.labels(row).iter().any(|l| match l {
                RefLabel::Branch { name, .. }
                | RefLabel::Remote { name }
                | RefLabel::Tag { name }
                | RefLabel::Stash { name }
                | RefLabel::Proposal { name } => name.to_lowercase().contains(&needle),
                RefLabel::Head => false,
            });
            if label_hit || id.to_string().starts_with(&needle) {
                hits.push(row);
                continue;
            }
            let commit = repo.find_commit(id).map_err(read_err)?;
            let subject = commit.message().map_err(read_err)?.summary().to_string();
            let author = commit.author().map_err(read_err)?.name.to_string();
            if subject.to_lowercase().contains(&needle) || author.to_lowercase().contains(&needle) {
                hits.push(row);
            }
        }
        Ok(hits)
    }

    /// Rows `start..start + count` with their commit metadata, clamped to the graph.
    ///
    /// # Errors
    /// [`Error::Read`] when a commit can't be read.
    pub fn rows(&self, repo: &Repo, start: usize, count: usize) -> Result<Vec<Row>, Error> {
        let repo = repo.gix();
        self.layout
            .rows(&self.topology, start, count)
            .into_iter()
            .zip(start..)
            .map(|(layout, row)| {
                if self.topology.is_wip(row) {
                    return Ok(Row {
                        id: String::new(),
                        short_id: String::new(),
                        subject: "Uncommitted changes".into(),
                        author: String::new(),
                        time: 0,
                        labels: Vec::new(),
                        wip: true,
                        after: None,
                        faded: false,
                        stash: None,
                        proposal: None,
                        layout,
                    });
                }
                if let Some(ghost) = self.topology.ghost(row) {
                    return Ok(Row {
                        id: String::new(),
                        short_id: String::new(),
                        subject: ghost.subject.clone(),
                        author: String::new(),
                        time: 0,
                        labels: Vec::new(),
                        wip: false,
                        after: Some(ghost.tag.clone()),
                        faded: false,
                        stash: None,
                        proposal: None,
                        layout,
                    });
                }
                let id = self.topology.id(row);
                let commit = repo.find_commit(id).map_err(read_err)?;
                let message = commit.message().map_err(read_err)?;
                let author = commit.author().map_err(read_err)?;
                Ok(Row {
                    id: id.to_string(),
                    short_id: id.to_hex_with_len(7).to_string(),
                    subject: message.summary().to_string(),
                    author: author.name.to_string(),
                    time: author.time().map_err(read_err)?.seconds,
                    labels: self.topology.labels(row).to_vec(),
                    wip: false,
                    after: None,
                    faded: self.topology.stroke(row) == Stroke::Faded,
                    stash: self.topology.stash(row),
                    proposal: self.topology.proposal(row).map(str::to_owned),
                    layout,
                })
            })
            .collect()
    }
}
