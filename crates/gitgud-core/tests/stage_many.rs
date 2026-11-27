use gitgud_core::session::Session;
use gitgud_core::testing::TestRepo;

/// Text with LF and CRLF endings, binaries, LFS files, deletions and edits: enough of each to
/// take the bulk path (blobs written ahead in one pack).
fn changes(repo: &TestRepo) {
    repo.git(&["config", "core.autocrlf", "true"]);
    repo.git(&["lfs", "install", "--local"]);
    repo.write(
        ".gitattributes",
        "*.png filter=lfs diff=lfs merge=lfs -text\n",
    );
    for i in 0..60 {
        repo.write(&format!("old/keep{i}.txt"), &format!("line {i}\n"));
        repo.write(&format!("old/gone{i}.txt"), "bye\n");
    }
    repo.commit_all("base");
    for i in 0..60 {
        repo.write(
            &format!("old/keep{i}.txt"),
            &format!("line {i}\r\nedited\r\n"),
        );
        repo.remove(&format!("old/gone{i}.txt"));
        repo.write(&format!("new/lf{i}.cs"), &format!("class A{i} {{}}\n"));
        repo.write(&format!("new/crlf{i}.cs"), &format!("class B{i} {{}}\r\n"));
        repo.write(&format!("new/tex{i}.png"), &format!("pixels {i}"));
        std::fs::write(
            repo.path().join(format!("new/bin{i}.dat")),
            [0u8, 1, 2, i as u8, 0, 255],
        )
        .unwrap();
    }
}

fn loose(repo: &TestRepo) -> usize {
    repo.git(&["count-objects", "-v"])
        .lines()
        .find_map(|l| l.strip_prefix("count: "))
        .unwrap()
        .parse()
        .unwrap()
}

fn index(repo: &TestRepo) -> String {
    repo.git(&["ls-files", "-s"])
}

#[test]
fn staging_many_files_gives_the_index_plain_git_add_gives() {
    let ours = TestRepo::init();
    let plain = TestRepo::init();
    changes(&ours);
    changes(&plain);

    let session = Session::default();
    let id = session.open(ours.path()).unwrap().id;
    let paths: Vec<String> = session
        .repo_status(id)
        .unwrap()
        .entries
        .into_iter()
        .map(|e| e.path)
        .collect();
    assert!(
        paths.len() >= 300,
        "{} paths: the bulk path needs 100+",
        paths.len()
    );
    let before = loose(&ours);
    session.stage(id, &paths).unwrap();
    plain.git(&["add", "-A"]);

    assert_eq!(index(&ours), index(&plain));
    assert_eq!(
        ours.git(&["status", "--porcelain"])
            .lines()
            .filter(|l| !l.starts_with(['M', 'A', 'D']))
            .count(),
        0,
        "everything staged:\n{}",
        ours.git(&["status", "--porcelain"])
    );
    // 60 LFS pointers are left to git add (loose); the 240 other new blobs went into the pack.
    let added = loose(&ours) - before;
    assert_eq!(added, 60, "loose objects the stage wrote");
    ours.git(&["fsck", "--no-progress"]);
}

#[test]
fn staging_a_few_files_stays_plain_git_add() {
    let repo = TestRepo::init();
    repo.write("a.txt", "a\n").commit_all("a");
    repo.write("a.txt", "b\n").write("c.txt", "c\n");
    let session = Session::default();
    let id = session.open(repo.path()).unwrap().id;
    session
        .stage(id, &["a.txt".into(), "c.txt".into()])
        .unwrap();
    assert_eq!(repo.git(&["status", "--porcelain"]), "M  a.txt\nA  c.txt\n");
    assert!(
        std::fs::read_dir(repo.path().join(".git/objects/pack"))
            .unwrap()
            .next()
            .is_none(),
        "no pack for a small stage"
    );
}
