use gitgud_core::graph::{RefLabel, Topology};
use gitgud_core::repo;
use gitgud_core::testing::TestRepo;

/// Walks with gix and checks order and parents against `git log --topo-order`.
fn topology(repo: &TestRepo) -> Topology {
    let t = Topology::walk(&repo::open(repo.path()).unwrap(), false).unwrap();
    let ours: Vec<(String, Vec<String>)> = (0..t.len())
        .map(|row| {
            let parents = t.parents(row).iter().map(|&p| t.id(p as usize).to_string());
            (t.id(row).to_string(), parents.collect())
        })
        .collect();
    assert_eq!(
        ours,
        repo.cli_topo(),
        "gix and git log --topo-order disagree"
    );
    t
}

fn commit(repo: &TestRepo, file: &str, msg: &str) {
    repo.write(file, msg).commit_all(msg);
}

fn row_of(repo: &TestRepo, t: &Topology, rev: &str) -> usize {
    let id = gix::ObjectId::from_hex(repo.git(&["rev-parse", rev]).trim().as_bytes()).unwrap();
    (0..t.len())
        .find(|&r| t.id(r) == id)
        .unwrap_or_else(|| panic!("{rev} not in topology"))
}

fn branch(name: &str, current: bool) -> RefLabel {
    RefLabel::Branch {
        name: name.into(),
        current,
    }
}

#[test]
fn linear_history() {
    let repo = TestRepo::init();
    for i in 0..5 {
        commit(&repo, "f.txt", &format!("c{i}"));
    }
    let t = topology(&repo);
    assert_eq!(t.len(), 5);
    assert_eq!(t.labels(0), [branch("main", true)]);
    assert!(t.parents(4).is_empty());
}

#[test]
fn branch_and_merge() {
    let repo = TestRepo::init();
    commit(&repo, "a.txt", "base");
    repo.git(&["checkout", "-q", "-b", "feature"]);
    commit(&repo, "f.txt", "f1");
    commit(&repo, "f.txt", "f2");
    repo.git(&["checkout", "-q", "main"]);
    commit(&repo, "m.txt", "m1");
    repo.git(&["merge", "-q", "--no-ff", "-m", "merge", "feature"]);

    let t = topology(&repo);
    assert_eq!(t.len(), 5);
    assert_eq!(t.parents(0).len(), 2);
    assert_eq!(
        t.labels(row_of(&repo, &t, "feature")),
        [branch("feature", false)]
    );
}

#[test]
fn unmerged_branches_and_tags() {
    let repo = TestRepo::init();
    commit(&repo, "a.txt", "base");
    repo.git(&["tag", "v1"]);
    repo.git(&["tag", "-a", "-m", "annotated", "v1-annotated"]);
    repo.git(&["checkout", "-q", "-b", "left"]);
    commit(&repo, "l.txt", "l1");
    repo.git(&["checkout", "-q", "main"]);
    repo.git(&["checkout", "-q", "-b", "right"]);
    commit(&repo, "r.txt", "r1");

    let t = topology(&repo);
    assert_eq!(t.len(), 3);
    assert_eq!(
        t.labels(row_of(&repo, &t, "v1")),
        [
            branch("main", false),
            RefLabel::Tag { name: "v1".into() },
            RefLabel::Tag {
                name: "v1-annotated".into()
            },
        ]
    );
    assert_eq!(
        t.labels(row_of(&repo, &t, "right")),
        [branch("right", true)]
    );
}

#[test]
fn octopus_merge() {
    let repo = TestRepo::init();
    commit(&repo, "a.txt", "base");
    for b in ["b1", "b2", "b3"] {
        repo.git(&["checkout", "-q", "-b", b, "main"]);
        commit(&repo, &format!("{b}.txt"), b);
    }
    repo.git(&["checkout", "-q", "main"]);
    commit(&repo, "m.txt", "m1");
    repo.git(&["merge", "-q", "-m", "octopus", "b1", "b2", "b3"]);

    let t = topology(&repo);
    assert_eq!(t.parents(0).len(), 4);
}

#[test]
fn criss_cross_merges() {
    let repo = TestRepo::init();
    commit(&repo, "a.txt", "base");
    repo.git(&["checkout", "-q", "-b", "other"]);
    commit(&repo, "o.txt", "o1");
    repo.git(&["checkout", "-q", "main"]);
    commit(&repo, "m.txt", "m1");
    repo.git(&["merge", "-q", "--no-ff", "-m", "main<-other", "other~0"]);
    repo.git(&["checkout", "-q", "other"]);
    repo.git(&["merge", "-q", "--no-ff", "-m", "other<-main", "main~1"]);
    commit(&repo, "o.txt", "o2");
    repo.git(&["checkout", "-q", "main"]);
    repo.git(&["merge", "-q", "--no-ff", "-m", "final", "other"]);

    topology(&repo);
}

#[test]
fn two_roots() {
    let repo = TestRepo::init();
    commit(&repo, "a.txt", "main root");
    repo.git(&["checkout", "-q", "--orphan", "pages"]);
    repo.git(&["rm", "-rqf", "."]);
    commit(&repo, "index.html", "pages root");

    let t = topology(&repo);
    let roots = (0..t.len()).filter(|&r| t.parents(r).is_empty()).count();
    assert_eq!(roots, 2);
}

#[test]
fn detached_head_off_any_branch() {
    let repo = TestRepo::init();
    commit(&repo, "a.txt", "base");
    repo.git(&["checkout", "-q", "--detach"]);
    commit(&repo, "a.txt", "detached work");

    let t = topology(&repo);
    assert_eq!(t.len(), 2);
    assert_eq!(t.labels(0), [RefLabel::Head]);
    assert_eq!(t.labels(1), [branch("main", false)]);
}

#[test]
fn remote_branches_without_their_symbolic_head() {
    let repo = TestRepo::init();
    commit(&repo, "a.txt", "base");
    commit(&repo, "a.txt", "local only");
    repo.git(&["update-ref", "refs/remotes/origin/main", "HEAD~1"]);
    repo.git(&[
        "symbolic-ref",
        "refs/remotes/origin/HEAD",
        "refs/remotes/origin/main",
    ]);

    let t = topology(&repo);
    assert_eq!(
        t.labels(1),
        [RefLabel::Remote {
            name: "origin/main".into()
        }]
    );
}

#[test]
fn includes_stashes_and_ignores_non_commit_tags() {
    let repo = TestRepo::init();
    commit(&repo, "a.txt", "base");
    repo.write("a.txt", "dirty");
    repo.git(&["stash", "-q"]);
    let tree = repo.git(&["rev-parse", "HEAD^{tree}"]);
    repo.git(&["tag", "tree-tag", tree.trim()]);

    let t = Topology::walk(&repo::open(repo.path()).unwrap(), false).unwrap();
    assert_eq!(t.len(), 2);
    assert_eq!(
        t.labels(0),
        [RefLabel::Stash {
            name: "stash@{0}".into()
        }]
    );
    assert_eq!(t.parents(0), [1]);
    assert_eq!(t.labels(1), [branch("main", true)]);
    let non_stash = vec![(t.id(1).to_string(), Vec::<String>::new())];
    assert_eq!(non_stash, repo.cli_topo());
}

#[test]
fn unborn_repo_is_empty() {
    let repo = TestRepo::init();
    let t = Topology::walk(&repo::open(repo.path()).unwrap(), false).unwrap();
    assert!(t.is_empty());
}

#[test]
fn parents_always_come_later() {
    let repo = TestRepo::init();
    commit(&repo, "a.txt", "base");
    for i in 0..4 {
        repo.git(&["checkout", "-q", "-b", &format!("b{i}"), "main"]);
        commit(&repo, "b.txt", &format!("b{i}"));
        repo.git(&["checkout", "-q", "main"]);
        commit(&repo, "m.txt", &format!("m{i}"));
        repo.git(&["merge", "-q", "--no-ff", "-m", "merge", &format!("b{i}")]);
    }
    let t = topology(&repo);
    for row in 0..t.len() {
        assert!(t.parents(row).iter().all(|&p| p as usize > row));
    }
}
