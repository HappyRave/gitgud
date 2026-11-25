use gitgud_core::git::{self, GitChoice, GitSource};
use gitgud_core::repo::Error;
use gitgud_core::session::Session;

#[test]
fn switching_git_saves_the_choice_and_swaps_the_session_git() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("git.json");
    // SAFETY: the only test in this binary, so no other thread reads the environment.
    unsafe { std::env::set_var("GITGUD_GIT_SETTINGS", &file) };

    let session = Session::default();
    let before = session.git().unwrap();

    let embedded = session.set_git_choice(&GitChoice::Embedded).unwrap();
    assert_eq!(embedded.source, GitSource::Bundled);
    assert_eq!(GitChoice::load(), GitChoice::Embedded);
    assert_eq!(git::locate().unwrap().path, embedded.path);
    assert!(!std::sync::Arc::ptr_eq(&before, &session.git().unwrap()));

    let custom = GitChoice::Custom {
        path: embedded.path.clone(),
    };
    assert_eq!(
        session.set_git_choice(&custom).unwrap().source,
        GitSource::Custom
    );
    assert_eq!(GitChoice::load(), custom);

    let missing = GitChoice::Custom {
        path: dir.path().join("nope.exe"),
    };
    assert!(matches!(
        session.set_git_choice(&missing),
        Err(Error::Git(git::Error::NotAFile(_)))
    ));
    assert_eq!(
        GitChoice::load(),
        custom,
        "a failed switch keeps the old choice"
    );

    session.set_git_choice(&GitChoice::Auto).unwrap();
    assert_eq!(GitChoice::load(), GitChoice::Auto);
}
