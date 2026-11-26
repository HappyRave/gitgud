use std::path::Path;
use std::process::Command;

use gitgud_core::overlap::Risk;
use gitgud_core::session::Session;
use gitgud_core::testing::TestRepo;

const ATTRIBUTES: &str = "*.unity merge=binary\n*.asset filter=lfs diff=lfs merge=lfs -text\n";
const CODE: &str = "one\ntwo\nthree\nfour\nfive\nsix\nseven\neight\nnine\nten\n";

fn bare() -> (tempfile::TempDir, String) {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("origin.git");
    let ok = Command::new("git")
        .args(["init", "-q", "--bare", "-b", "main"])
        .arg(&path)
        .status()
        .unwrap()
        .success();
    assert!(ok);
    (dir, path.to_str().unwrap().replace('\\', "/"))
}

fn clone_of(url: &str, name: &str) -> TestRepo {
    let repo = TestRepo::init();
    repo.git(&["config", "user.name", name]);
    repo.git(&["lfs", "install", "--local"]);
    repo.git(&["remote", "add", "origin", url]);
    repo.git(&["fetch", "-q", "origin"]);
    repo.git(&["checkout", "-q", "-B", "main", "--track", "origin/main"]);
    repo
}

/// Commits with the real clock, so `--since` windows see it as recent.
fn commit_now(repo: &Path, message: &str) {
    for args in [vec!["add", "-A"], vec!["commit", "-q", "-m", message]] {
        let ok = Command::new("git")
            .args(&args)
            .current_dir(repo)
            .status()
            .unwrap()
            .success();
        assert!(ok);
    }
}

/// Origin with a scene, an LFS asset and two code files; JG and Max clone it.
fn team() -> (tempfile::TempDir, TestRepo, TestRepo) {
    let (dir, url) = bare();
    let seed = TestRepo::init();
    seed.git(&["lfs", "install", "--local"]);
    seed.write(".gitattributes", ATTRIBUTES)
        .write("Assets/Lobby.unity", "scene v1\n")
        .write("Assets/Lobby.unity.meta", "guid: 1\n")
        .write("Assets/Catalog.asset", "catalog v1\n")
        .write("Assets/Code.cs", CODE)
        .write("Assets/Other.cs", CODE)
        .write("Assets/Theirs.cs", "theirs\n")
        .write("Assets/Mine.cs", "mine\n")
        .commit_all("seed");
    seed.git(&["remote", "add", "origin", &url]);
    seed.git(&["push", "-q", "origin", "main"]);
    let jg = clone_of(&url, "JG");
    let max = clone_of(&url, "Max");
    (dir, jg, max)
}

#[test]
fn flags_what_both_sides_changed_with_how_it_will_merge() {
    let (_dir, jg, max) = team();
    jg.write("Assets/Lobby.unity", "scene by JG\n")
        .write("Assets/Lobby.unity.meta", "guid: 1\nlabels: [jg]\n")
        .write("Assets/Catalog.asset", "catalog by JG\n")
        .write("Assets/Code.cs", &CODE.replace("two", "TWO by JG"))
        .write("Assets/Other.cs", &CODE.replace("one", "ONE by JG"))
        .write("Assets/Theirs.cs", "changed\n")
        .commit_all("JG - lobby, catalog, code");
    jg.git(&["push", "-q", "origin", "main"]);

    max.write("Assets/Catalog.asset", "catalog by Max\n")
        .commit_all("MD - catalog");
    max.write("Assets/Lobby.unity", "scene by Max\n")
        .write("Assets/Code.cs", &CODE.replace("two", "two by Max"))
        .write("Assets/Other.cs", &CODE.replace("ten", "ten by Max"))
        .write("Assets/Mine.cs", "changed\n");
    let session = Session::default();
    let id = session.open(max.path()).unwrap().id;

    let before = session.overlap(id).unwrap().unwrap();
    assert!(before.files.is_empty(), "nothing fetched yet: {before:?}");

    max.git(&["fetch", "-q", "origin"]);
    let status_before = max.git(&["status", "--porcelain"]);
    let got = session.overlap(id).unwrap().unwrap();
    assert_eq!(
        max.git(&["status", "--porcelain"]),
        status_before,
        "the radar writes nothing to the work tree"
    );
    assert_eq!(got.against, ["origin/main"]);
    let rows: Vec<(&str, Risk, bool)> = got
        .files
        .iter()
        .map(|f| (f.path.as_str(), f.risk, f.dirty))
        .collect();
    assert_eq!(rows.len(), 4, "{rows:?}");
    assert!(rows.contains(&("Assets/Lobby.unity", Risk::WontMerge, true)));
    assert!(rows.contains(&("Assets/Catalog.asset", Risk::WontMerge, false)));
    assert_eq!(rows[2], ("Assets/Code.cs", Risk::MayConflict, true));
    assert_eq!(rows[3], ("Assets/Other.cs", Risk::Merges, true));
    let lobby = &got.files[0..2]
        .iter()
        .find(|f| f.path == "Assets/Lobby.unity")
        .unwrap();
    assert_eq!(lobby.authors, ["JG"]);
    assert_eq!(lobby.on, "origin/main");
    assert_eq!(lobby.commits, 1);
    assert_eq!(
        lobby.newest_commit,
        max.git(&["rev-parse", "origin/main"]).trim()
    );
}

#[test]
fn a_merged_pull_clears_the_overlap() {
    let (_dir, jg, max) = team();
    jg.write("Assets/Code.cs", &CODE.replace("one", "ONE"))
        .commit_all("JG - code");
    jg.git(&["push", "-q", "origin", "main"]);
    max.write("Assets/Code.cs", &CODE.replace("ten", "TEN"))
        .commit_all("MD - code");
    max.git(&["fetch", "-q", "origin"]);
    let session = Session::default();
    let id = session.open(max.path()).unwrap().id;
    assert_eq!(session.overlap(id).unwrap().unwrap().files.len(), 1);

    max.git(&["merge", "-q", "--no-edit", "origin/main"]);
    assert!(session.overlap(id).unwrap().unwrap().files.is_empty());
}

#[test]
fn a_feature_branch_also_hears_about_recent_main_changes() {
    let (_dir, jg, max) = team();
    max.git(&["checkout", "-q", "-b", "Feature"]);
    max.git(&["push", "-q", "-u", "origin", "Feature"]);
    max.write("Assets/Lobby.unity", "scene by Max\n")
        .write("Assets/Mine.cs", "old news\n");

    jg.write("Assets/Mine.cs", "JG long ago\n")
        .commit_all("JG - old change");
    jg.write("Assets/Lobby.unity", "scene by JG\n");
    commit_now(jg.path(), "JG - lobby today");
    jg.git(&["push", "-q", "origin", "main"]);
    max.git(&["fetch", "-q", "origin"]);

    let session = Session::default();
    let id = session.open(max.path()).unwrap().id;
    let got = session.overlap(id).unwrap().unwrap();
    assert_eq!(got.against, ["origin/Feature", "origin/main"]);
    let paths: Vec<&str> = got.files.iter().map(|f| f.path.as_str()).collect();
    assert_eq!(
        paths,
        ["Assets/Lobby.unity"],
        "main's changes older than a week aren't news"
    );
    assert_eq!(got.files[0].on, "origin/main");
}

/// Times the radar on a real repo as last fetched:
/// `GITGUD_OVERLAP_CORPUS=<repo> cargo test -p gitgud-core --test overlap -- --ignored --nocapture`.
#[test]
#[ignore = "needs GITGUD_OVERLAP_CORPUS"]
fn runs_on_a_real_repo() {
    let root = std::env::var("GITGUD_OVERLAP_CORPUS").expect("set GITGUD_OVERLAP_CORPUS");
    let session = Session::default();
    let id = session.open(&root).unwrap().id;
    let started = std::time::Instant::now();
    let got = session.overlap(id).unwrap();
    println!("{root}: {:?}", started.elapsed());
    println!("{got:#?}");
}

#[test]
fn nothing_to_compare_with_without_a_remote() {
    let repo = TestRepo::init();
    repo.write("a.txt", "a\n").commit_all("a");
    let session = Session::default();
    let id = session.open(repo.path()).unwrap().id;
    assert_eq!(session.overlap(id).unwrap(), None);
}
