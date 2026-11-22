use gitgud_core::cli::GitCli;
use gitgud_core::git::locate;
use gitgud_core::oplog::OpLog;
use gitgud_core::testing::TestRepo;

#[test]
fn file_history_follow_blame_and_outside_reflog_and_commit_graph() {
    let repo = TestRepo::init();
    repo.write("src/old.txt", "line 1\nline 2\nline 3\nline 4\nline 5\n")
        .commit_all("create old");
    repo.git(&["mv", "src/old.txt", "src/new.txt"]);
    repo.write(
        "src/new.txt",
        "line 1\nline 2 edited\nline 3\nline 4\nline 5\nline 6\n",
    )
    .commit_all("rename and edit");

    let git = GitCli::new(&locate().unwrap());
    let opened = gitgud_core::repo::open(repo.path()).unwrap();

    let hist = opened.file_history(&git, "src/new.txt", 20).unwrap();
    assert_eq!(hist.len(), 2);
    assert_eq!(hist[0].subject, "rename and edit");
    assert_eq!(hist[1].subject, "create old");

    let blame = opened.blame_file(&git, "src/new.txt", None).unwrap();
    assert_eq!(blame.len(), 6);
    assert_eq!(blame[0].summary, "create old");
    assert_eq!(blame[1].summary, "rename and edit");
    assert_eq!(blame[5].text, "line 6");

    let reflog = opened.outside_reflog(&git, 10).unwrap();
    assert!(reflog.iter().any(|r| r.action.contains("rename and edit")));

    assert!(!opened.has_commit_graph());
    opened.write_commit_graph(&git).unwrap();
    assert!(opened.has_commit_graph());

    let stats = OpLog::for_repo(&opened).stats().unwrap();
    assert_eq!(stats.entry_count, 0);
}
