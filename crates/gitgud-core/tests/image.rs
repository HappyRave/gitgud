use gitgud_core::image::Version;
use gitgud_core::repo::{self, Error};
use gitgud_core::session::Session;
use gitgud_core::testing::TestRepo;

const V1: &[u8] = b"\x89PNG\0one";
const V2: &[u8] = b"\x89PNG\0two";
const V3: &[u8] = b"\x89PNG\0three";

fn put(repo: &TestRepo, path: &str, data: &[u8]) {
    std::fs::write(repo.path().join(path), data).unwrap();
}

fn head(repo: &TestRepo) -> String {
    repo.git(&["rev-parse", "HEAD"]).trim().to_owned()
}

#[test]
fn reads_each_version_of_an_image() {
    let repo = TestRepo::init();
    put(&repo, "t.png", V1);
    repo.commit_all("one");
    put(&repo, "t.png", V2);
    repo.commit_all("two");
    put(&repo, "t.png", V3);
    repo.git(&["add", "t.png"]);
    put(&repo, "t.png", b"\x89PNG\0disk");
    let r = repo::open(repo.path()).unwrap();
    let id = head(&repo);

    assert_eq!(
        r.image_bytes("t.png", &Version::Commit(id.clone()))
            .unwrap(),
        V2
    );
    assert_eq!(r.image_bytes("t.png", &Version::Parent(id)).unwrap(), V1);
    assert_eq!(r.image_bytes("t.png", &Version::Head).unwrap(), V2);
    assert_eq!(r.image_bytes("t.png", &Version::Index).unwrap(), V3);
    assert_eq!(
        r.image_bytes("t.png", &Version::Worktree).unwrap(),
        b"\x89PNG\0disk"
    );
}

#[test]
fn refuses_non_images_and_missing_versions() {
    let repo = TestRepo::init();
    repo.write("a.txt", "a\n").commit_all("first");
    put(&repo, "new.png", V1);
    let r = repo::open(repo.path()).unwrap();
    let id = head(&repo);

    assert!(matches!(
        r.image_bytes("a.txt", &Version::Head),
        Err(Error::Refused(_))
    ));
    assert!(matches!(
        r.image_bytes("new.png", &Version::Head),
        Err(Error::Refused(_))
    ));
    assert!(matches!(
        r.image_bytes("new.png", &Version::Parent(id)),
        Err(Error::Refused(_))
    ));
    assert_eq!(r.image_bytes("new.png", &Version::Worktree).unwrap(), V1);

    std::fs::write(repo.path().join("../outside.png"), V2).unwrap();
    for path in ["../outside.png", "C:/Windows/outside.png", "/etc/x.png"] {
        assert!(
            matches!(
                r.image_bytes(path, &Version::Worktree),
                Err(Error::Refused(_))
            ),
            "{path}"
        );
    }
    std::fs::remove_file(repo.path().join("../outside.png")).unwrap();
}

#[test]
fn lfs_pointers_resolve_to_the_local_object() {
    let repo = TestRepo::init();
    let oid = "ab".repeat(32);
    let pointer = format!("version https://git-lfs.github.com/spec/v1\noid sha256:{oid}\nsize 9\n");
    repo.write("t.png", &pointer).commit_all("lfs");
    let r = repo::open(repo.path()).unwrap();

    assert!(matches!(
        r.image_bytes("t.png", &Version::Head),
        Err(Error::Refused(m)) if m.contains("isn't downloaded")
    ));

    let dir = repo.path().join(".git/lfs/objects/ab/ab");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join(&oid), V1).unwrap();
    assert_eq!(r.image_bytes("t.png", &Version::Head).unwrap(), V1);
}

#[test]
fn reads_both_sides_of_a_conflict() {
    let repo = TestRepo::init();
    put(&repo, "t.png", V1);
    repo.commit_all("base");
    repo.git(&["checkout", "-q", "-b", "other"]);
    put(&repo, "t.png", V3);
    repo.commit_all("theirs");
    repo.git(&["checkout", "-q", "main"]);
    put(&repo, "t.png", V2);
    repo.commit_all("yours");
    let session = Session::default();
    let id = session.open(repo.path()).unwrap().id;
    session.merge(id, "other").unwrap();
    let r = session.get(id).unwrap().repo.clone();

    assert_eq!(r.image_bytes("t.png", &Version::Yours).unwrap(), V2);
    assert_eq!(r.image_bytes("t.png", &Version::Theirs).unwrap(), V3);
}
