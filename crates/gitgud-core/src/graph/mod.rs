mod layout;
mod topology;

pub use layout::{Layout, MAX_LANES, Part, RowLayout, Segment};
pub use topology::{RefLabel, Topology};

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
                    layout,
                })
            })
            .collect()
    }
}
