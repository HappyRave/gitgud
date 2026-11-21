use std::path::Path;

use gitgud_core::git::{Error, MIN_GIT, locate, locate_in};
use tempfile::TempDir;

/// A directory holding a fake `git` that prints `output` for `--version`.
fn fake_git(output: &str) -> TempDir {
    let dir = tempfile::tempdir().unwrap();
    write_script(dir.path(), output);
    dir
}

#[cfg(windows)]
fn write_script(dir: &Path, output: &str) {
    std::fs::write(dir.join("git.cmd"), format!("@echo {output}\r\n")).unwrap();
}

#[cfg(not(windows))]
fn write_script(dir: &Path, output: &str) {
    use std::os::unix::fs::PermissionsExt;
    let path = dir.join("git");
    std::fs::write(&path, format!("#!/bin/sh\necho '{output}'\n")).unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
}

#[test]
fn finds_the_real_git() {
    let info = locate().expect("a git >= 2.45.2 must be on PATH to run the tests");
    assert!(info.version >= MIN_GIT);
    assert!(info.path.is_file(), "{} is not a file", info.path.display());
}

#[test]
fn accepts_a_recent_git() {
    let dir = fake_git("git version 2.60.0.windows.1");
    let info = locate_in(dir.path()).unwrap();
    assert_eq!(info.version.to_string(), "2.60.0");
    assert!(info.path.starts_with(dir.path()));
}

#[test]
fn accepts_exactly_the_floor() {
    let dir = fake_git("git version 2.45.2");
    assert_eq!(locate_in(dir.path()).unwrap().version, MIN_GIT);
}

#[test]
fn rejects_an_old_git() {
    let dir = fake_git("git version 2.45.1");
    match locate_in(dir.path()) {
        Err(Error::TooOld { found, min }) => {
            assert_eq!(found.to_string(), "2.45.1");
            assert_eq!(min, MIN_GIT);
        }
        other => panic!("expected TooOld, got {other:?}"),
    }
}

#[test]
fn rejects_unexpected_output() {
    let dir = fake_git("not git at all");
    assert!(matches!(
        locate_in(dir.path()),
        Err(Error::UnparsableVersion(_))
    ));
}

#[test]
fn reports_missing_git() {
    let empty = tempfile::tempdir().unwrap();
    assert!(matches!(locate_in(empty.path()), Err(Error::NotFound)));
}
