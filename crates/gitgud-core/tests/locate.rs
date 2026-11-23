use std::path::Path;

use gitgud_core::git::{
    Error, GitSource, GitVersion, MIN_GIT, MIN_GIT_LFS, locate, locate_in, locate_with_fallback,
};
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

#[cfg(windows)]
fn fake_git_exe(dir: &TempDir) -> std::path::PathBuf {
    dir.path().join("git.cmd")
}

#[cfg(not(windows))]
fn fake_git_exe(dir: &TempDir) -> std::path::PathBuf {
    dir.path().join("git")
}

#[test]
fn system_git_wins_over_bundled_when_recent() {
    let sys = fake_git("git version 2.50.0");
    let bundled = fake_git("git version 2.56.1");
    let info = locate_with_fallback(sys.path(), &[fake_git_exe(&bundled)]).unwrap();
    assert_eq!(info.source, GitSource::System);
    assert_eq!(info.version.to_string(), "2.50.0");
}

#[test]
fn falls_back_to_bundled_when_system_git_is_too_old_or_missing() {
    let old = fake_git("git version 2.45.1");
    let empty = tempfile::tempdir().unwrap();
    let bundled = fake_git("git version 2.56.1.windows.1");
    let exe = fake_git_exe(&bundled);

    let from_old = locate_with_fallback(old.path(), std::slice::from_ref(&exe)).unwrap();
    assert_eq!(from_old.source, GitSource::Bundled);
    assert_eq!(from_old.version.to_string(), "2.56.1");

    let from_missing = locate_with_fallback(empty.path(), std::slice::from_ref(&exe)).unwrap();
    assert_eq!(from_missing.source, GitSource::Bundled);

    // Without a valid bundled candidate, the original system error stands.
    let old_err =
        locate_with_fallback(old.path(), &[empty.path().join("missing.exe")]).unwrap_err();
    assert!(matches!(old_err, Error::TooOld { .. }));
}

#[test]
fn parses_lfs_version_and_checks_floor() {
    let v = GitVersion::parse_lfs("git-lfs/3.8.1 (GitHub; windows amd64; go 1.21.7)").unwrap();
    assert_eq!(v.to_string(), "3.8.1");
    assert!(v >= MIN_GIT_LFS);

    let old = GitVersion::parse_lfs("git-lfs/3.2.9").unwrap();
    assert!(old < MIN_GIT_LFS);
    assert!(GitVersion::parse_lfs("not lfs").is_err());
}
