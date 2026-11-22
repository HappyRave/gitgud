use gitgud_core::cli::GitCli;
use gitgud_core::git::locate;
use gitgud_core::lfs::{lfs_info, lfs_lock, lfs_locks_list, lfs_unlock};
use gitgud_core::repo::Error;
use gitgud_core::testing::TestRepo;

#[test]
fn lfs_parses_attributes_and_disables_locks_on_bitbucket_cloud() {
    let repo = TestRepo::init();
    repo.write(
        ".gitattributes",
        "*.psd filter=lfs diff=lfs merge=lfs -text\n*.png filter=lfs diff=lfs merge=lfs -text\n",
    )
    .commit_all("attributes");
    repo.git(&[
        "remote",
        "add",
        "origin",
        "https://bitbucket.org/team/game.git",
    ]);

    let git = GitCli::new(&locate().unwrap());
    let opened = gitgud_core::repo::open(repo.path()).unwrap();

    let info = lfs_info(&git, &opened).unwrap();
    assert_eq!(
        info.tracked_patterns,
        vec!["*.png".to_owned(), "*.psd".to_owned()]
    );
    assert!(!info.supports_locks);
    assert!(
        info.locks_unsupported_reason
            .as_deref()
            .unwrap()
            .contains("Bitbucket Cloud")
    );

    assert!(lfs_locks_list(&git, &opened).unwrap().is_empty());
    let err = lfs_lock(&git, &opened, "Art/Hero.psd").unwrap_err();
    assert!(matches!(err, Error::Refused(_)));
    let err2 = lfs_unlock(&git, &opened, "Art/Hero.psd", false).unwrap_err();
    assert!(matches!(err2, Error::Refused(_)));
}
