use gitgud_core::graph::{Graph, Part, RefLabel, Row};
use gitgud_core::repo::{self, Error};
use gitgud_core::session::Session;
use gitgud_core::testing::TestRepo;

fn history() -> TestRepo {
    let repo = TestRepo::init();
    repo.write("a.txt", "base").commit_all("base commit");
    repo.git(&["checkout", "-q", "-b", "feature"]);
    repo.write("f.txt", "f")
        .commit_all("feature work\n\nwith a body");
    repo.git(&["checkout", "-q", "main"]);
    repo.write("m.txt", "m").commit_all("main work");
    repo.git(&["merge", "-q", "--no-ff", "-m", "Merge feature", "feature"]);
    repo.git(&["tag", "v1"]);
    repo
}

fn rows(repo: &TestRepo, wip: bool) -> Vec<Row> {
    let r = repo::open(repo.path()).unwrap();
    let graph = Graph::build(&r, wip).unwrap();
    graph.rows(&r, 0, graph.len()).unwrap()
}

#[test]
fn metadata_matches_git_log() {
    let repo = history();
    let cli: Vec<Vec<String>> = repo
        .git(&[
            "log",
            "--topo-order",
            "--branches",
            "--tags",
            "HEAD",
            "--format=%H%x1f%h%x1f%s%x1f%an%x1f%at",
        ])
        .lines()
        .map(|l| l.split('\x1f').map(str::to_owned).collect())
        .collect();
    let ours: Vec<Vec<String>> = rows(&repo, false)
        .into_iter()
        .map(|r| vec![r.id, r.short_id, r.subject, r.author, r.time.to_string()])
        .collect();
    assert_eq!(ours, cli);
}

#[test]
fn rows_carry_labels_and_layout() {
    let repo = history();
    let rows = rows(&repo, false);
    assert_eq!(
        rows[0].labels,
        [
            RefLabel::Branch {
                name: "main".into(),
                current: true
            },
            RefLabel::Tag { name: "v1".into() },
        ]
    );
    assert_eq!(
        rows[0]
            .layout
            .segments
            .iter()
            .filter(|s| s.part == Part::Bottom)
            .count(),
        2
    );
    assert!(rows.iter().all(|r| !r.wip));
}

#[test]
fn wip_row_sits_on_top_of_head() {
    let repo = history();
    repo.git(&["checkout", "-q", "feature"]);
    let rows = rows(&repo, true);
    assert!(rows[0].wip);
    assert_eq!(rows[0].subject, "Uncommitted changes");
    assert!(rows[0].id.is_empty());

    let head = repo.git(&["rev-parse", "HEAD"]);
    let head_row = rows.iter().position(|r| r.id == head.trim()).unwrap();
    let wip_lane = rows[0].layout.node;
    // The WIP line runs straight down its lane into HEAD's dot.
    for row in &rows[1..head_row] {
        assert!(
            row.layout
                .segments
                .iter()
                .any(|s| s.part == Part::Through && s.from == wip_lane)
        );
    }
    assert!(
        rows[head_row]
            .layout
            .segments
            .iter()
            .any(|s| s.part == Part::Top && s.from == wip_lane)
    );
}

#[test]
fn find_matches_subject_author_id_prefix_and_labels() {
    let repo = history();
    repo.write("dirty.txt", "x");
    let r = repo::open(repo.path()).unwrap();
    let graph = Graph::build(&r, true).unwrap();
    let all = graph.rows(&r, 0, graph.len()).unwrap();
    let subjects = |hits: Vec<usize>| -> Vec<String> {
        hits.into_iter().map(|i| all[i].subject.clone()).collect()
    };

    let mut work = graph.find(&r, "WORK").unwrap();
    assert!(work.is_sorted(), "graph order");
    work.sort_by_key(|&i| all[i].subject.clone());
    assert_eq!(subjects(work), ["feature work", "main work"]);
    assert_eq!(subjects(graph.find(&r, "v1").unwrap()), ["Merge feature"]);
    let feature_tip = all
        .iter()
        .position(|row| row.subject == "feature work")
        .unwrap();
    assert!(graph.find(&r, "feature").unwrap().contains(&feature_tip));
    let id = &all[feature_tip].id;
    assert_eq!(
        graph.find(&r, &id[..8]).unwrap(),
        [feature_tip],
        "an id prefix"
    );
    assert_eq!(
        graph.find(&r, "gitgud tests").unwrap().len(),
        4,
        "the author, never the WIP row"
    );
    assert!(graph.find(&r, "  ").unwrap().is_empty());
    assert!(graph.find(&r, "nothing like this").unwrap().is_empty());
}

#[test]
fn windows_match_the_full_list() {
    let repo = history();
    let r = repo::open(repo.path()).unwrap();
    let graph = Graph::build(&r, true).unwrap();
    let full = graph.rows(&r, 0, graph.len()).unwrap();
    assert_eq!(graph.rows(&r, 2, 2).unwrap(), full[2..4]);
    assert_eq!(graph.rows(&r, 3, 100).unwrap(), full[3..]);
    assert!(graph.rows(&r, 100, 5).unwrap().is_empty());
}

#[test]
fn session_adds_wip_only_when_dirty() {
    let repo = history();
    let session = Session::default();

    let clean = session.open(repo.path()).unwrap();
    assert_eq!(clean.rows, 4);

    repo.write("a.txt", "changed");
    let dirty = session.open(repo.path()).unwrap();
    assert_eq!(dirty.rows, 5);
    assert_ne!(clean.id, dirty.id);

    let open = session.get(dirty.id).unwrap();
    assert!(open.graph.rows(&open.repo, 0, 1).unwrap()[0].wip);
}

#[test]
fn session_opens_an_unborn_repo_without_rows() {
    let repo = TestRepo::init();
    repo.write("first.txt", "1");
    let opened = Session::default().open(repo.path()).unwrap();
    assert_eq!(opened.rows, 0);
}

#[test]
fn session_get_and_close() {
    let repo = history();
    let session = Session::default();
    let opened = session.open(repo.path()).unwrap();
    assert_eq!(
        opened.root.canonicalize().unwrap(),
        repo.path().canonicalize().unwrap()
    );

    assert!(session.get(opened.id).is_ok());
    session.close(opened.id).unwrap();
    assert!(matches!(session.get(opened.id), Err(Error::UnknownRepo(id)) if id == opened.id));
    assert!(matches!(
        session.close(opened.id),
        Err(Error::UnknownRepo(_))
    ));
    assert!(matches!(session.get(999), Err(Error::UnknownRepo(999))));
}

#[test]
fn refresh_keeps_the_graph_while_history_is_unchanged() {
    let repo = history();
    repo.write("a.txt", "dirty");
    let session = Session::default();
    let id = session.open(repo.path()).unwrap().id;
    let before = session.get(id).unwrap().graph;

    repo.git(&["add", "a.txt"]);
    repo.write("b.txt", "more");
    let opened = session.refresh(id).unwrap();

    assert_eq!(opened.graph_version, 0);
    assert!(std::sync::Arc::ptr_eq(
        &before,
        &session.get(id).unwrap().graph
    ));
}

#[test]
fn refresh_rebuilds_after_a_commit() {
    let repo = history();
    let session = Session::default();
    let opened = session.open(repo.path()).unwrap();

    repo.write("a.txt", "new").commit_all("one more");
    let refreshed = session.refresh(opened.id).unwrap();

    assert_eq!(refreshed.id, opened.id);
    assert_eq!(refreshed.rows, opened.rows + 1);
    assert_eq!(refreshed.graph_version, 1);
    let open = session.get(opened.id).unwrap();
    assert_eq!(
        open.graph.rows(&open.repo, 0, 1).unwrap()[0].subject,
        "one more"
    );
}

#[test]
fn refresh_adds_and_drops_the_wip_row() {
    let repo = history();
    let session = Session::default();
    let id = session.open(repo.path()).unwrap().id;

    repo.write("a.txt", "dirty");
    let dirty = session.refresh(id).unwrap();
    assert_eq!((dirty.rows, dirty.graph_version), (5, 1));

    repo.git(&["checkout", "--", "a.txt"]);
    let clean = session.refresh(id).unwrap();
    assert_eq!((clean.rows, clean.graph_version), (4, 2));
}

#[test]
fn refresh_rebuilds_when_refs_move() {
    let repo = history();
    let session = Session::default();
    let id = session.open(repo.path()).unwrap().id;

    for change in [
        &["tag", "v2", "HEAD~1"][..],
        &["branch", "topic"],
        &["checkout", "-q", "topic"],
        &["checkout", "-q", "--detach"],
        &["update-ref", "refs/remotes/origin/main", "HEAD"],
    ] {
        let before = session.get(id).unwrap().graph_version;
        repo.git(change);
        assert_eq!(
            session.refresh(id).unwrap().graph_version,
            before + 1,
            "{change:?}"
        );
    }
}

#[test]
fn refresh_of_an_unknown_repo_fails() {
    assert!(matches!(
        Session::default().refresh(3),
        Err(Error::UnknownRepo(3))
    ));
}

#[test]
fn session_locates_git_once() {
    let session = Session::default();
    let first: *const _ = session.git().unwrap();
    let second: *const _ = session.git().unwrap();
    assert_eq!(first, second);
}

#[test]
fn session_rejects_a_plain_folder() {
    let dir = tempfile::tempdir().unwrap();
    assert!(matches!(
        Session::default().open(dir.path()),
        Err(Error::NotARepo(_))
    ));
}
