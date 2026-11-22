use gitgud_core::cli::GitCli;
use gitgud_core::git::locate;
use gitgud_core::graph::Graph;
use gitgud_core::proposals::{
    CreateProposalInput, ProposalStatus, agent_status, create_proposal, get_proposal,
    list_proposals, reject_proposal, request_changes, withdraw_proposal,
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
