use std::time::Duration;

use gitgud_core::cli::GitCli;
use gitgud_core::git::locate;
use gitgud_core::graph::Graph;
use gitgud_core::proposals::{
    CreateProposalInput, ProposalStatus, agent_status, create_proposal, get_proposal,
    list_proposals, reject_proposal, request_changes, wait_for_proposal, withdraw_proposal,
};
use gitgud_core::repo;
use gitgud_core::session::Session;
use gitgud_core::testing::TestRepo;

#[test]
fn proposals_lifecycle_untracked_hunks_replay_and_undo() {
    let repo = TestRepo::init();
    repo.write(
        "ProjectSettings/ProjectVersion.txt",
        "m_EditorVersion: 6000.0.12f1\n",
    )
    .write("src.txt", "line 1\nline 2\nline 3\nline 10\n")
    .commit_all("initial");

    // Agent modifies tracked file and adds an untracked Unity C# file without `.meta`.
    repo.write("src.txt", "line 1\nline 2\nline 3\nline 10 edited\n");
    repo.write("Assets/NewScript.cs", "class NewScript {}\n");

    let git = GitCli::new(&locate().unwrap());
    let opened = repo::open(repo.path()).unwrap();

    // agent_status detects both tracked and untracked hunks without mutating `.git/index`.
    let st = agent_status(&opened, &git).unwrap();
    assert_eq!(st.files.len(), 2);
    assert!(
        st.risk_flags
            .iter()
            .any(|f| f.kind == "missingMeta" && f.path == "Assets/NewScript.cs")
    );
    let src_file = st.files.iter().find(|f| f.path == "src.txt").unwrap();
    assert_eq!(src_file.hunks.len(), 1);
    let hunk_id = src_file.hunks[0].id.clone();

    let created = create_proposal(
        &opened,
        &git,
        &CreateProposalInput {
            message: "feat: edit line 10".into(),
            why: Some("fixes edge condition".into()),
            agent: Some("claude".into()),
            paths: vec![],
            hunk_ids: vec![hunk_id],
        },
    )
    .unwrap();
    assert_eq!(created.status, ProposalStatus::Pending);
    assert_eq!(created.files, vec!["src.txt"]);

    // Max's real index stays clean (staged count 0).
    let real_status = opened.status().unwrap();
    assert!(real_status.entries.iter().all(|e| e.index.is_none()));

    // Graph splices the active proposal as a dashed ghost row with a proposal label.
    let g = Graph::build(&opened, true).unwrap();
    let rows = g.rows(&opened, 0, g.len()).unwrap();
    assert!(
        rows.iter()
            .any(|r| r.proposal.as_deref() == Some(created.id.as_str()))
    );

    // Request changes adds comment readable by the agent; move HEAD in parallel to test replay.
    let updated = request_changes(&opened, &created.id, "looks good after rebase", None).unwrap();
    assert_eq!(updated.status, ProposalStatus::ChangesRequested);
    assert_eq!(
        get_proposal(&opened, &created.id).unwrap().comments.len(),
        1
    );

    // Move HEAD cleanly on a different file/line so `head_moved` is true and Approve must replay.
    repo.write("other.txt", "parallel work\n");
    repo.git(&["add", "other.txt"]);
    repo.git(&["commit", "-m", "parallel head commit"]);
    let listed = list_proposals(&opened).unwrap();
    assert!(
        listed
            .iter()
            .find(|p| p.id == created.id)
            .unwrap()
            .head_moved
    );

    // Approve via Session so it commits cleanly onto moved HEAD and is Ctrl+Z undoable.
    let session = Session::default();
    let sid = session.open(repo.path()).unwrap().id;
    session
        .proposal_approve(sid, &created.id, Some("feat: replayed line 10"), &[])
        .unwrap();
    assert_eq!(
        repo.git(&["log", "-1", "--format=%s"]).trim(),
        "feat: replayed line 10"
    );
    assert_eq!(
        get_proposal(&opened, &created.id).unwrap().status,
        ProposalStatus::Approved
    );

    // Undo brings HEAD back to `parallel head commit`.
    session.undo(sid).unwrap();
    assert_eq!(
        repo.git(&["log", "-1", "--format=%s"]).trim(),
        "parallel head commit"
    );

    // Test withdraw & reject delete hidden refs cleanly.
    let p2 = create_proposal(
        &opened,
        &git,
        &CreateProposalInput {
            message: "feat: withdraw test".into(),
            why: None,
            agent: None,
            paths: vec!["Assets/NewScript.cs".into()],
            hunk_ids: vec![],
        },
    )
    .unwrap();
    let withdrawn = withdraw_proposal(&opened, &git, &p2.id).unwrap();
    assert_eq!(withdrawn.status, ProposalStatus::Withdrawn);

    let p3 = create_proposal(
        &opened,
        &git,
        &CreateProposalInput {
            message: "feat: reject test".into(),
            why: None,
            agent: None,
            paths: vec!["Assets/NewScript.cs".into()],
            hunk_ids: vec![],
        },
    )
    .unwrap();
    let rejected = reject_proposal(&opened, &git, &p3.id, Some("duplicate")).unwrap();
    assert_eq!(rejected.status, ProposalStatus::Rejected);
    assert_eq!(rejected.comments[0].text, "duplicate");
}

#[test]
fn hunk_ids_survive_nearby_line_shifts_between_status_and_propose() {
    let repo = TestRepo::init();
    let initial: String = (1..=20).map(|i| format!("line {i}\n")).collect();
    repo.write("shift.txt", &initial).commit_all("base");

    let bottom_edit = initial.replace("line 18\n", "line 18 fixed\n");
    repo.write("shift.txt", &bottom_edit);

    let git = GitCli::new(&locate().unwrap());
    let opened = repo::open(repo.path()).unwrap();
    let st1 = agent_status(&opened, &git).unwrap();
    let hunk_id = st1.files[0].hunks[0].id.clone();

    // Max inserts lines above the agent's hunk, shifting its line numbers without changing it.
    let shifted = format!("top a\ntop b\n{bottom_edit}");
    repo.write("shift.txt", &shifted);
    let st2 = agent_status(&opened, &git).unwrap();
    let hunks = &st2.files[0].hunks;
    assert_eq!(hunks.len(), 2);
    assert_eq!(hunks[1].id, hunk_id);

    // Partial hunk matches must refuse rather than silently dropping unknown IDs.
    let partial_err = create_proposal(
        &opened,
        &git,
        &CreateProposalInput {
            message: "mixed hunks".into(),
            why: None,
            agent: Some("claude".into()),
            paths: vec![],
            hunk_ids: vec![hunk_id.clone(), "deadbeef0000".into()],
        },
    )
    .unwrap_err();
    match partial_err {
        gitgud_core::repo::Error::Refused(msg) => assert!(msg.contains("deadbeef0000"), "{msg}"),
        other => panic!("expected Refused error for unknown hunk ID, got {other:?}"),
    }

    let prop = create_proposal(
        &opened,
        &git,
        &CreateProposalInput {
            message: "fix line 18 only".into(),
            why: None,
            agent: Some("claude".into()),
            paths: vec![],
            hunk_ids: vec![hunk_id.clone()],
        },
    )
    .unwrap();
    let proposed_content = repo.git(&["show", &format!("{}:shift.txt", prop.commit_id)]);
    assert!(!proposed_content.contains("top a"));
    assert!(proposed_content.contains("line 18 fixed"));

    // Editing inside the hunk itself invalidates its ID.
    repo.write(
        "shift.txt",
        &shifted.replace("line 18 fixed", "line 18 rewritten"),
    );
    assert!(
        create_proposal(
            &opened,
            &git,
            &CreateProposalInput {
                message: "stale hunk".into(),
                why: None,
                agent: None,
                paths: vec![],
                hunk_ids: vec![hunk_id],
            },
        )
        .is_err()
    );
}

#[test]
fn wait_for_proposal_returns_on_verdict_and_times_out_when_pending() {
    let repo = TestRepo::init();
    repo.write("a.txt", "before\n").commit_all("base");
    repo.write("a.txt", "after\n");

    let git = GitCli::new(&locate().unwrap());
    let opened = repo::open(repo.path()).unwrap();
    let created = create_proposal(
        &opened,
        &git,
        &CreateProposalInput {
            message: "pending item".into(),
            why: None,
            agent: Some("claude".into()),
            paths: vec!["a.txt".into()],
            hunk_ids: vec![],
        },
    )
    .unwrap();

    let timed_out = wait_for_proposal(&opened, &created.id, 0, Duration::from_millis(20)).unwrap();
    assert!(timed_out.timed_out);
    assert_eq!(timed_out.proposal.status, ProposalStatus::Pending);

    let path = repo.path().to_owned();
    let pid = created.id.clone();
    let updater = std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(75));
        let opened2 = repo::open(&path).unwrap();
        request_changes(&opened2, &pid, "please rename", None).unwrap();
    });

    let done = wait_for_proposal(&opened, &created.id, 3, Duration::from_millis(20)).unwrap();
    updater.join().unwrap();
    assert!(!done.timed_out);
    assert_eq!(done.proposal.status, ProposalStatus::ChangesRequested);
    assert_eq!(done.proposal.comments[0].text, "please rename");
}

fn propose(opened: &gitgud_core::repo::Repo, git: &GitCli, path: &str) -> String {
    create_proposal(
        opened,
        git,
        &CreateProposalInput {
            message: format!("agent: {path}"),
            why: None,
            agent: Some("claude".into()),
            paths: vec![path.into()],
            hunk_ids: vec![],
        },
    )
    .unwrap()
    .id
}

#[test]
fn approve_refuses_while_other_changes_are_staged() {
    let repo = TestRepo::init();
    repo.write("a.txt", "a\n")
        .write("mine.txt", "mine\n")
        .commit_all("base");
    repo.write("a.txt", "a agent\n");
    let git = GitCli::new(&locate().unwrap());
    let opened = repo::open(repo.path()).unwrap();
    let id = propose(&opened, &git, "a.txt");

    repo.write("mine.txt", "mine staged\n");
    repo.git(&["add", "mine.txt"]);
    let session = Session::default();
    let sid = session.open(repo.path()).unwrap().id;
    let err = session.proposal_approve(sid, &id, None, &[]).unwrap_err();
    assert!(
        matches!(&err, gitgud_core::repo::Error::Refused(m) if m.contains("Unstage")),
        "{err:?}"
    );
    assert_eq!(repo.git(&["log", "-1", "--format=%s"]).trim(), "base");
    assert_eq!(
        repo.git(&["diff", "--cached", "--name-only"]).trim(),
        "mine.txt"
    );
    assert_eq!(
        get_proposal(&opened, &id).unwrap().status,
        ProposalStatus::Pending
    );
}

#[test]
fn approve_that_cannot_replay_leaves_the_index_as_it_was() {
    let repo = TestRepo::init();
    repo.write("a.txt", "one\n").commit_all("base");
    repo.write("a.txt", "agent\n");
    let git = GitCli::new(&locate().unwrap());
    let opened = repo::open(repo.path()).unwrap();
    let id = propose(&opened, &git, "a.txt");

    repo.write("a.txt", "mine\n").commit_all("mine");
    let session = Session::default();
    let sid = session.open(repo.path()).unwrap().id;
    let err = session.proposal_approve(sid, &id, None, &[]).unwrap_err();
    assert!(
        matches!(err, gitgud_core::repo::Error::Refused(_)),
        "{err:?}"
    );
    assert_eq!(repo.git(&["ls-files", "-u"]), "");
    assert!(repo.git_ok(&["diff", "--cached", "--quiet"]));
    assert_eq!(repo.git(&["log", "-1", "--format=%s"]).trim(), "mine");
}

#[test]
fn approve_keeps_non_utf8_bytes() {
    let repo = TestRepo::init();
    repo.write("latin1.txt", "caf\n").commit_all("base");
    let latin1 = b"caf\xe9\n";
    std::fs::write(repo.path().join("latin1.txt"), latin1).unwrap();
    let git = GitCli::new(&locate().unwrap());
    let opened = repo::open(repo.path()).unwrap();
    let id = propose(&opened, &git, "latin1.txt");

    // HEAD moves so approval has to replay the patch.
    repo.write("other.txt", "x\n");
    repo.git(&["add", "other.txt"]);
    repo.git(&["commit", "-m", "other"]);
    let session = Session::default();
    let sid = session.open(repo.path()).unwrap().id;
    session.proposal_approve(sid, &id, None, &[]).unwrap();
    let blob = std::process::Command::new("git")
        .args(["show", "HEAD:latin1.txt"])
        .current_dir(repo.path())
        .output()
        .unwrap()
        .stdout;
    assert_eq!(blob, latin1);
}
