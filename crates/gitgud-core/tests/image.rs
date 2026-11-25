use std::time::Instant;

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
fn large_unhydrated_lfs_pointer_refuses_auto_fetch_and_prompts_fetch() {
    let repo = TestRepo::init();
    let oid = "cd".repeat(32);
    let large_size = gitgud_core::lfs::AUTO_FETCH_MAX_BYTES + 1024;
    let pointer = format!(
        "version https://git-lfs.github.com/spec/v1\noid sha256:{oid}\nsize {large_size}\n"
    );
    repo.write("big.png", &pointer).commit_all("large lfs");
    repo.git(&["remote", "add", "origin", "https://example.invalid/r.git"]);
    let r = repo::open(repo.path()).unwrap();

    let err = r
        .image_bytes_with_fetch(
            None,
            "big.png",
            &Version::Head,
            gitgud_core::lfs::AUTO_FETCH_MAX_BYTES,
        )
        .unwrap_err();
    assert!(
        matches!(err, Error::Refused(m) if m.contains("click Fetch")),
        "unexpected error for >25 MiB pointer"
    );
}

#[test]
fn reopening_a_repo_keeps_its_caches_until_its_config_changes() {
    let repo = TestRepo::init();
    repo.write("Assets/boat.prefab", "%YAML 1.1\n")
        .commit_all("one");
    let session = Session::default();
    let first = session.open(repo.path()).unwrap().id;
    session
        .model_info(first, "Assets/boat.prefab", &Version::Head)
        .unwrap();

    let again = session.open(repo.path()).unwrap().id;
    assert_ne!(first, again);
    assert!(std::sync::Arc::ptr_eq(
        &session.get(first).unwrap().repo,
        &session.get(again).unwrap().repo
    ));
    assert_eq!(session.cache_stats(again).unwrap().model_infos, 1);

    repo.git(&["config", "user.name", "Someone Else"]);
    let fresh = session.open(repo.path()).unwrap().id;
    assert_eq!(
        session.cache_stats(fresh).unwrap().model_infos,
        0,
        "a changed config opens it anew"
    );
}

#[test]
fn warms_textures_in_the_background() {
    let repo = TestRepo::init();
    let mut png = std::io::Cursor::new(Vec::new());
    image::RgbaImage::from_fn(1200, 64, |x, _| image::Rgba([x as u8, 0, 0, 255]))
        .write_to(&mut png, image::ImageFormat::Png)
        .unwrap();
    std::fs::create_dir_all(repo.path().join("Assets")).unwrap();
    put(&repo, "Assets/wall.png", png.get_ref());
    repo.commit_all("one");
    let session = Session::default();
    let id = session.open(repo.path()).unwrap().id;
    session
        .warm_textures(
            id,
            Version::Head,
            vec!["Assets/wall.png".into(), "Assets/gone.png".into()],
            512,
            true,
        )
        .unwrap();
    let start = Instant::now();
    while session.cache_stats(id).unwrap().preview_textures == 0 {
        assert!(start.elapsed().as_secs() < 20, "nothing was warmed");
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    let r = session.get(id).unwrap().repo.clone();
    assert!(r.compressed_texture(png.get_ref(), 512).is_some());
    assert!(matches!(
        session.warm_textures(99, Version::Head, vec![], 512, true),
        Err(Error::UnknownRepo(99))
    ));
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

#[test]
fn decodes_2048px_tga_and_large_flat_psd_within_spec_targets() {
    let repo = TestRepo::init();

    // 2048x2048 32-bit uncompressed TGA (16 MiB pixel payload).
    let w_tga = 2048usize;
    let h_tga = 2048usize;
    let mut tga = vec![0u8; 18 + w_tga * h_tga * 4];
    tga[2] = 2;
    tga[12..14].copy_from_slice(&(w_tga as u16).to_le_bytes());
    tga[14..16].copy_from_slice(&(h_tga as u16).to_le_bytes());
    tga[16] = 32;
    tga[17] = 0x20;
    tga[18..22].copy_from_slice(&[10, 20, 30, 255]);
    std::fs::write(repo.path().join("big.tga"), &tga).unwrap();

    // 5000x5000 4-channel 8-bit flat RGB PSD (~100 MB raw pixel payload, compression 0).
    let w_psd = 5000usize;
    let h_psd = 5000usize;
    let psd_header = 26 + 4 + 4 + 4 + 2;
    let mut psd = vec![0u8; psd_header + w_psd * h_psd * 4];
    psd[0..4].copy_from_slice(b"8BPS");
    psd[4..6].copy_from_slice(&1u16.to_be_bytes());
    psd[12..14].copy_from_slice(&4u16.to_be_bytes());
    psd[14..18].copy_from_slice(&(h_psd as u32).to_be_bytes());
    psd[18..22].copy_from_slice(&(w_psd as u32).to_be_bytes());
    psd[22..24].copy_from_slice(&8u16.to_be_bytes());
    psd[24..26].copy_from_slice(&3u16.to_be_bytes());
    std::fs::write(repo.path().join("big.psd"), &psd).unwrap();

    let r = repo::open(repo.path()).unwrap();

    let t0 = Instant::now();
    let tga_png = r.image_bytes("big.tga", &Version::Worktree).unwrap();
    let tga_ms = t0.elapsed().as_secs_f64() * 1000.0;
    assert!(tga_png.starts_with(b"\x89PNG\r\n\x1a\n"));

    let t1 = Instant::now();
    let psd_png = r.image_bytes("big.psd", &Version::Worktree).unwrap();
    let psd_ms = t1.elapsed().as_secs_f64() * 1000.0;
    assert!(psd_png.starts_with(b"\x89PNG\r\n\x1a\n"));

    println!("perf image decode: 2048px TGA={tga_ms:.1} ms, 100MB flat PSD={psd_ms:.1} ms");
    #[cfg(not(debug_assertions))]
    {
        assert!(tga_ms <= 300.0, "2048px TGA took {tga_ms:.1} ms > 300 ms");
        assert!(
            psd_ms <= 1500.0,
            "100 MB flat PSD took {psd_ms:.1} ms > 1500 ms"
        );
    }
    #[cfg(debug_assertions)]
    {
        assert!(tga_ms <= 3000.0, "debug 2048px TGA took {tga_ms:.1} ms");
        assert!(
            psd_ms <= 15000.0,
            "debug 100 MB flat PSD took {psd_ms:.1} ms"
        );
    }
}
