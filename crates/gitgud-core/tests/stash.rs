use gitgud_core::graph::RefLabel;
use gitgud_core::session::Session;
use gitgud_core::testing::TestRepo;

fn read(repo: &TestRepo, path: &str) -> Option<String> {
    std::fs::read_to_string(repo.path().join(path)).ok()
}

#[test]
fn a_stash_shows_the_same_entry_apply_acts_on_when_one_cant_be_read() {
    let repo = TestRepo::init();
    repo.write("a.txt", "base\n").commit_all("init");
    for name in ["oldest", "middle", "newest"] {
        repo.write("a.txt", &format!("{name}\n"));
        repo.git(&["stash", "push", "-q", "-m", name]);
    }
    // stash@{1}'s line names an object that isn't there.
    let log = repo.path().join(".git/logs/refs/stash");
    let text = std::fs::read_to_string(&log).unwrap();
    let mut lines: Vec<String> = text.lines().map(str::to_owned).collect();
    let mut parts: Vec<String> = lines[1].split(' ').map(str::to_owned).collect();
    parts[1] = "f".repeat(40);
    lines[1] = parts.join(" ");
    std::fs::write(&log, lines.join("\n") + "\n").unwrap();
    let r = gitgud_core::repo::open(repo.path()).unwrap();

    let listed: Vec<(usize, String)> = r
        .stashes()
        .unwrap()
        .into_iter()
        .map(|e| (e.index, e.message))
        .collect();
    assert_eq!(listed, [(0, "newest".into()), (2, "oldest".into())]);
    let detail = r.stash_detail(2).unwrap();
    assert!(detail.subject.contains("oldest"), "{}", detail.subject);
}

#[test]
fn stash_save_inspect_apply_pop_drop_and_undo_all() {
    let repo = TestRepo::init();
    repo.write("a.txt", "base\n").commit_all("init");
    repo.write("a.txt", "edited\n")
        .write("new.txt", "untracked\n");

    let session = Session::default();
    let id = session.open(repo.path()).unwrap().id;

    session.stash_save(id, Some("wip ui"), true).unwrap();
    assert_eq!(read(&repo, "a.txt").unwrap(), "base\n");
    assert_eq!(read(&repo, "new.txt"), None);

    let open = session.get(id).unwrap();
    let stashes = open.repo.stashes().unwrap();
    assert_eq!(stashes.len(), 1);
    assert_eq!(stashes[0].message, "wip ui");
    assert_eq!(stashes[0].branch, "main");

    let detail = open.repo.stash_detail(0).unwrap();
    let names: Vec<_> = detail.files.iter().map(|f| f.path.as_str()).collect();
    assert_eq!(names, ["a.txt", "new.txt"]);
    let diff = open.repo.stash_file_diff(0, "new.txt").unwrap();
    assert_eq!(diff.hunks[0].lines[0].text, "untracked");

    // Stash shows up as a graph row with a stash label.
    let rows = open.graph.rows(&open.repo, 0, 10).unwrap();
    let stash_row = rows.iter().find(|r| r.stash == Some(0)).unwrap();
    assert!(stash_row.labels.contains(&RefLabel::Stash {
        name: "stash@{0}".into()
    }));

    // Undo stash save -> working copy restored and stash removed.
    session.undo(id).unwrap();
    assert_eq!(read(&repo, "a.txt").unwrap(), "edited\n");
    assert_eq!(read(&repo, "new.txt").unwrap(), "untracked\n");
    assert!(session.get(id).unwrap().repo.stashes().unwrap().is_empty());

    // Redo stash save -> stash back, worktree clean.
    session.redo(id).unwrap();
    assert_eq!(session.get(id).unwrap().repo.stashes().unwrap().len(), 1);
    assert_eq!(read(&repo, "a.txt").unwrap(), "base\n");

    // Apply, undo apply, then pop, undo pop, then drop, undo drop.
    session.stash_apply(id, 0).unwrap();
    assert_eq!(read(&repo, "a.txt").unwrap(), "edited\n");
    assert_eq!(read(&repo, "new.txt").unwrap(), "untracked\n");
    session.undo(id).unwrap();
    assert_eq!(read(&repo, "a.txt").unwrap(), "base\n");

    session.stash_pop(id, 0).unwrap();
    assert_eq!(read(&repo, "a.txt").unwrap(), "edited\n");
    assert!(session.get(id).unwrap().repo.stashes().unwrap().is_empty());

    session.undo(id).unwrap();
    assert_eq!(read(&repo, "a.txt").unwrap(), "base\n");
    assert_eq!(session.get(id).unwrap().repo.stashes().unwrap().len(), 1);

    session.stash_drop(id, 0).unwrap();
    assert!(session.get(id).unwrap().repo.stashes().unwrap().is_empty());
    session.undo(id).unwrap();
    assert_eq!(session.get(id).unwrap().repo.stashes().unwrap().len(), 1);
}
