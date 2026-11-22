use gitgud_core::session::Session;
use gitgud_core::testing::TestRepo;

#[test]
fn detects_stacked_branches_builds_compare_urls_restacks_and_undoes() {
    let tr = TestRepo::init();
    tr.write("base.txt", "base\n").commit_all("base");
    tr.git(&["remote", "add", "origin", "https://github.com/Max/boat.git"]);

    tr.git(&["checkout", "-b", "stack/part1"]);
    tr.write("p1.txt", "one\n").commit_all("part 1 work");
    let orig_part1 = tr.git(&["rev-parse", "stack/part1"]).trim().to_owned();

    tr.git(&["checkout", "-b", "stack/part2"]);
    tr.write("p2.txt", "two\n").commit_all("part 2 work");
    let orig_part2 = tr.git(&["rev-parse", "stack/part2"]).trim().to_owned();

    let session = Session::default();
    let opened = session.open(tr.path()).unwrap();
    let stacks = session.branch_stacks(opened.id).unwrap();
    assert_eq!(stacks.len(), 1);
    assert_eq!(stacks[0].root, "main");
    assert_eq!(stacks[0].levels.len(), 2);
    assert_eq!(stacks[0].levels[0].branch, "stack/part1");
    assert_eq!(stacks[0].levels[0].base, "main");
    assert_eq!(stacks[0].levels[0].commits, 1);
    assert!(!stacks[0].levels[0].needs_restack);
    assert_eq!(
        stacks[0].levels[0].pr_url.as_deref(),
        Some("https://github.com/Max/boat/compare/main...stack/part1?expand=1")
    );
    assert_eq!(stacks[0].levels[1].branch, "stack/part2");
    assert_eq!(stacks[0].levels[1].base, "stack/part1");
    assert!(stacks[0].levels[1].current);
    assert!(!stacks[0].levels[1].needs_restack);
    assert_eq!(
        stacks[0].levels[1].pr_url.as_deref(),
        Some("https://github.com/Max/boat/compare/stack/part1...stack/part2?expand=1")
    );

    // Advance main so the stack becomes stale and needs restack.
    tr.git(&["checkout", "main"]);
    tr.write("upstream.txt", "new\n").commit_all("advance main");
    tr.git(&["checkout", "stack/part2"]);
    session.refresh(opened.id).unwrap();

    let stale = session.branch_stacks(opened.id).unwrap();
    assert_eq!(stale.len(), 1);
    assert!(stale[0].levels[0].needs_restack);
    assert!(stale[0].levels[1].needs_restack);

    session.restack(opened.id, "stack/part2", "main").unwrap();
    let fresh = session.branch_stacks(opened.id).unwrap();
    assert_eq!(fresh.len(), 1);
    assert!(!fresh[0].levels[0].needs_restack);
    assert!(!fresh[0].levels[1].needs_restack);
    let new_part1 = tr.git(&["rev-parse", "stack/part1"]).trim().to_owned();
    let new_part2 = tr.git(&["rev-parse", "stack/part2"]).trim().to_owned();
    assert_ne!(new_part1, orig_part1);
    assert_ne!(new_part2, orig_part2);

    // Ctrl+Z undoes the entire restack cleanly.
    session.undo(opened.id).unwrap();
    assert_eq!(tr.git(&["rev-parse", "stack/part1"]).trim(), orig_part1);
    assert_eq!(tr.git(&["rev-parse", "stack/part2"]).trim(), orig_part2);
}
