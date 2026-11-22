use gitgud_core::cli::GitCli;
use gitgud_core::git::locate;
use gitgud_core::session::Session;
use gitgud_core::testing::TestRepo;

#[test]
fn tags_create_list_delete_and_remote_round_trip() {
    let origin = TestRepo::init();
    origin.write("a.txt", "1").commit_all("initial");

    let bare_dir = tempfile::tempdir().unwrap();
    let bare = bare_dir.path().join("origin.git");
    origin.git(&["clone", "-q", "--bare", ".", bare.to_str().unwrap()]);
    origin.git(&["remote", "add", "origin", bare.to_str().unwrap()]);

    let session = Session::default();
    let id = session.open(origin.path()).unwrap().id;
    let git = GitCli::new(&locate().unwrap());
    let repo = &session.get(id).unwrap().repo;

    repo.create_tag(&git, "v0.1.0", None, None).unwrap();
    repo.create_tag(&git, "v1.0.0", None, Some("First release"))
        .unwrap();

    let list = repo.tags().unwrap();
    assert_eq!(list.len(), 2);
    let v1 = list.iter().find(|t| t.name == "v1.0.0").unwrap();
    assert!(v1.annotated);
    assert_eq!(v1.message.as_deref(), Some("First release"));

    repo.push_tag(&git, "origin", "v1.0.0").unwrap();
    let remote_tags = origin.git(&["ls-remote", "--tags", "origin"]);
    assert!(remote_tags.contains("refs/tags/v1.0.0"));

    repo.delete_remote_tag(&git, "origin", "v1.0.0").unwrap();
    let after_remote = origin.git(&["ls-remote", "--tags", "origin"]);
    assert!(!after_remote.contains("refs/tags/v1.0.0"));

    repo.delete_tag(&git, "v0.1.0").unwrap();
    assert_eq!(repo.tags().unwrap().len(), 1);
}
