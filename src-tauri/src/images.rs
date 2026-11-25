use gitgud_core::image::{PreviewTexture, Version, image_type};
use gitgud_core::model::{model_kind, model_mime};
use gitgud_core::preview::file_mime;
use gitgud_core::repo::Error;
use gitgud_core::session::Session;
use std::time::{Duration, Instant};
use tauri::Url;
use tauri::http::{Response, StatusCode, header};

/// Answers `gitgud://localhost/image?repo=1&path=a.png&at=index` (on Windows
/// `http://gitgud.localhost/image?...`) with the image bytes, so the webview can
/// show a version of an image without base64 over IPC. `at` is `head`,
/// `index`, `worktree`, `yours`, `theirs`, or `commit` / `parent` with `commit=<id>`.
/// `/model` serves a 3D model the same way (with `mesh=<fileID>`, a Mesh stored inside
/// a scene or prefab, as GLB); `/ref?path=<model>&ref=<name>`
/// serves the file that model refers to as `ref` (a texture, a glTF buffer);
/// `/file` serves any file as stored (audio, video, PDF, fonts, data); `/audio`
/// a sound file decoded to WAV, for formats the webview can't play (AIFF).
///
/// `arrived` is when the request came in; the `Server-Timing` header gives `queue`
/// (waiting for a thread), `read` (the file's bytes as stored), `shrink` (texture work,
/// reading an LFS object and decoding included when the texture wasn't kept) and `total`.
pub fn respond_timed(session: &Session, uri: &str, arrived: Instant) -> Response<Vec<u8>> {
    let started = Instant::now();
    let mut steps = Steps::default();
    let answer = read(session, uri, &mut steps);
    let timing = format!(
        "queue;dur={:.1}, read;dur={:.1}, shrink;dur={:.1}, total;dur={:.1}",
        ms(started - arrived),
        ms(steps.read),
        ms(steps.shrink),
        ms(started.elapsed()),
    );
    match answer {
        Ok((bytes, mime)) => Response::builder()
            .header(header::CONTENT_TYPE, mime)
            .header(header::CONTENT_LENGTH, bytes.len())
            .header(header::CACHE_CONTROL, "no-store")
            .header(header::ACCESS_CONTROL_ALLOW_ORIGIN, "*")
            .header(
                header::ACCESS_CONTROL_EXPOSE_HEADERS,
                "Content-Length, Server-Timing",
            )
            .header("Server-Timing", timing)
            .body(bytes),
        Err((status, message)) => Response::builder()
            .status(status)
            .header(header::CONTENT_TYPE, "text/plain; charset=utf-8")
            .header(header::ACCESS_CONTROL_ALLOW_ORIGIN, "*")
            .body(message.into_bytes()),
    }
    .expect("static headers are valid")
}

#[cfg(test)]
pub fn respond(session: &Session, uri: &str) -> Response<Vec<u8>> {
    respond_timed(session, uri, Instant::now())
}

#[derive(Default)]
struct Steps {
    read: Duration,
    shrink: Duration,
}

fn ms(d: Duration) -> f64 {
    d.as_secs_f64() * 1000.0
}

/// Runs `f`, adding its time to `into`.
fn timed<T>(into: &mut Duration, f: impl FnOnce() -> T) -> T {
    let t = Instant::now();
    let out = f();
    *into += t.elapsed();
    out
}

fn read(
    session: &Session,
    uri: &str,
    steps: &mut Steps,
) -> Result<(Vec<u8>, &'static str), (StatusCode, String)> {
    let bad = |m: &str| (StatusCode::BAD_REQUEST, m.to_owned());
    let url = Url::parse(uri).map_err(|e| bad(&e.to_string()))?;
    let param = |name: &str| {
        url.query_pairs()
            .find(|(k, _)| k == name)
            .map(|(_, v)| v.into_owned())
    };
    let repo: u32 = param("repo")
        .and_then(|r| r.parse().ok())
        .ok_or_else(|| bad("missing repo"))?;
    let path = param("path").ok_or_else(|| bad("missing path"))?;
    let commit = || param("commit").ok_or_else(|| bad("missing commit"));
    let version = match param("at").as_deref() {
        Some("head") => Version::Head,
        Some("index") => Version::Index,
        Some("worktree") => Version::Worktree,
        Some("yours") => Version::Yours,
        Some("theirs") => Version::Theirs,
        Some("commit") => Version::Commit(commit()?),
        Some("parent") => Version::Parent(commit()?),
        _ => return Err(bad("unknown version")),
    };
    let max = param("max").and_then(|m| m.parse::<u32>().ok());
    let bc = param("gpu").as_deref() == Some("bc");
    match url.path() {
        "/model" if param("meshes").is_some() => {
            let ids = param("meshes").unwrap_or_default();
            let ids: Vec<&str> = ids.split(',').filter(|s| !s.is_empty()).collect();
            let bytes = timed(&mut steps.read, || {
                session.inline_meshes(repo, &path, &version, &ids)
            })
            .map_err(fail)?;
            Ok((bytes, "application/octet-stream"))
        }
        "/model" if param("mesh").is_some() => {
            let mesh = param("mesh").unwrap_or_default();
            let bytes = timed(&mut steps.read, || {
                session.inline_mesh(repo, &path, &version, &mesh)
            })
            .map_err(fail)?;
            Ok((bytes, "model/gltf-binary"))
        }
        "/model" => {
            let kind = model_kind(&path).ok_or_else(|| bad("not a 3D model"))?;
            let bytes = timed(&mut steps.read, || {
                session.model_bytes(repo, &path, &version)
            })
            .map_err(fail)?;
            Ok((bytes, model_mime(kind)))
        }
        "/file" => {
            let bytes = session.file_bytes(repo, &path, &version).map_err(fail)?;
            Ok((bytes, file_mime(&path)))
        }
        "/audio" => {
            let bytes = session.audio_wav(repo, &path, &version).map_err(fail)?;
            Ok((bytes, "audio/wav"))
        }
        "/ref" => {
            let reference = param("ref").ok_or_else(|| bad("missing ref"))?;
            let mime = image_type(&reference).unwrap_or("application/octet-stream");
            let bytes = timed(&mut steps.read, || {
                session.model_ref_bytes(repo, &path, &reference, &version)
            })
            .map_err(fail)?;
            timed(&mut steps.shrink, || {
                shrunk(session, repo, bytes, mime, max, bc)
            })
        }
        _ => {
            let mime = image_type(&path).ok_or_else(|| bad("not an image"))?;
            let auto_fetch = param("autofetch").as_deref() != Some("0");
            let Some(max) = max else {
                let bytes = timed(&mut steps.read, || {
                    session.image_bytes(repo, &path, &version, auto_fetch)
                })
                .map_err(fail)?;
                return Ok((bytes, mime));
            };
            // Kept textures are found by the file as stored, so a hit decodes nothing.
            let stored = timed(&mut steps.read, || {
                session.stored_image(repo, &path, &version)
            })
            .map_err(fail)?;
            let texture = timed(&mut steps.shrink, || {
                session.preview_texture(repo, &path, &version, &stored, max, bc, auto_fetch)
            })
            .map_err(fail)?;
            Ok(match texture {
                PreviewTexture::Dds(dds) => (dds, "image/vnd-ms.dds"),
                PreviewTexture::Png(png) => (png, "image/png"),
                PreviewTexture::Original(bytes) => (bytes, mime),
            })
        }
    }
}

/// With `max=<px>` (3D previews), an image bigger than that comes back shrunk, as PNG;
/// with `gpu=bc` too, GPU-compressed as DDS when it can be.
fn shrunk(
    session: &Session,
    repo: u32,
    bytes: Vec<u8>,
    mime: &'static str,
    max: Option<u32>,
    bc: bool,
) -> Result<(Vec<u8>, &'static str), (StatusCode, String)> {
    let Some(max) = max else {
        return Ok((bytes, mime));
    };
    if bc
        && let Some(dds) = session
            .compressed_texture(repo, &bytes, max)
            .map_err(fail)?
    {
        return Ok((dds, "image/vnd-ms.dds"));
    }
    Ok(
        match session.shrink_texture(repo, &bytes, max).map_err(fail)? {
            Some(small) => (small, "image/png"),
            None => (bytes, mime),
        },
    )
}

fn fail(e: Error) -> (StatusCode, String) {
    let status = match e {
        Error::Refused(_) | Error::UnknownRepo(_) => StatusCode::NOT_FOUND,
        _ => StatusCode::INTERNAL_SERVER_ERROR,
    };
    (status, e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use gitgud_core::testing::TestRepo;

    fn body(r: &Response<Vec<u8>>) -> String {
        String::from_utf8_lossy(r.body()).into_owned()
    }

    #[test]
    fn serves_an_image_version_with_its_type() {
        let repo = TestRepo::init();
        std::fs::write(repo.path().join("t.png"), b"\x89PNG\0one").unwrap();
        repo.commit_all("one");
        std::fs::write(repo.path().join("t.png"), b"\x89PNG\0two").unwrap();
        let session = Session::default();
        let id = session.open(repo.path()).unwrap().id;
        let head = repo.git(&["rev-parse", "HEAD"]).trim().to_owned();

        let r = respond(
            &session,
            &format!("http://gitgud.localhost/image?repo={id}&path=t.png&at=commit&commit={head}"),
        );
        assert_eq!(r.status(), StatusCode::OK);
        assert_eq!(r.headers()[header::CONTENT_TYPE], "image/png");
        assert_eq!(r.headers()[header::ACCESS_CONTROL_ALLOW_ORIGIN], "*");
        assert_eq!(r.body(), b"\x89PNG\0one");
        let timing = r.headers()["Server-Timing"].to_str().unwrap();
        assert!(
            timing.starts_with("queue;dur=")
                && timing.contains(", read;dur=")
                && timing.contains(", total;dur="),
            "{timing}"
        );

        let r = respond(
            &session,
            &format!("gitgud://localhost/image?repo={id}&path=t.png&at=worktree"),
        );
        assert_eq!(r.body(), b"\x89PNG\0two");

        assert_eq!(image_type("albedo.dds"), Some("image/png"));
        assert_eq!(image_type("light.exr"), Some("image/png"));
        assert_eq!(image_type("sky.hdr"), Some("image/png"));
        assert_eq!(image_type("mask.tiff"), Some("image/png"));
    }

    #[test]
    fn serves_models_and_the_textures_they_refer_to() {
        let repo = TestRepo::init();
        repo.write("Art/boat.gltf", "{\"asset\":{}}")
            .write("Art/Textures/hull.png", "\u{89}PNG-hull")
            .write("Art/boat.bin", "buffer")
            .commit_all("one");
        repo.write("Art/boat.gltf", "{\"asset\":{\"v\":2}}");
        let session = Session::default();
        let id = session.open(repo.path()).unwrap().id;

        let r = respond(
            &session,
            &format!("http://gitgud.localhost/model?repo={id}&path=Art%2Fboat.gltf&at=head"),
        );
        assert_eq!(r.status(), StatusCode::OK);
        assert_eq!(r.headers()[header::CONTENT_TYPE], "model/gltf+json");
        assert_eq!(r.headers()[header::CONTENT_LENGTH], "12");
        assert_eq!(r.body(), b"{\"asset\":{}}");
        let r = respond(
            &session,
            &format!("http://gitgud.localhost/model?repo={id}&path=Art%2Fboat.gltf&at=worktree"),
        );
        assert_eq!(r.body(), b"{\"asset\":{\"v\":2}}");

        let r = respond(
            &session,
            &format!(
                "http://gitgud.localhost/ref?repo={id}&path=Art%2Fboat.gltf&ref=C%3A%5Cart%5Chull.png&at=head"
            ),
        );
        assert_eq!(r.status(), StatusCode::OK);
        assert_eq!(r.headers()[header::CONTENT_TYPE], "image/png");
        assert_eq!(r.body(), "\u{89}PNG-hull".as_bytes());

        let pdf = respond(
            &session,
            &format!("http://gitgud.localhost/file?repo={id}&path=Art%2Fboat.bin&at=head"),
        );
        assert_eq!(pdf.status(), StatusCode::OK);
        assert_eq!(pdf.body(), b"buffer");
        assert_eq!(
            pdf.headers()[header::CONTENT_TYPE],
            "application/octet-stream"
        );

        let bin = respond(
            &session,
            &format!(
                "http://gitgud.localhost/ref?repo={id}&path=Art%2Fboat.gltf&ref=boat.bin&at=head"
            ),
        );
        assert_eq!(bin.status(), StatusCode::OK);
        assert_eq!(
            bin.headers()[header::CONTENT_TYPE],
            "application/octet-stream"
        );
        assert_eq!(bin.body(), b"buffer");
        let missing = respond(
            &session,
            &format!(
                "http://gitgud.localhost/ref?repo={id}&path=Art%2Fboat.gltf&ref=deck.png&at=head"
            ),
        );
        assert_eq!(missing.status(), StatusCode::NOT_FOUND);
        assert!(body(&missing).contains("deck.png"));
        let not_model = respond(
            &session,
            &format!(
                "http://gitgud.localhost/model?repo={id}&path=Art%2FTextures%2Fhull.png&at=head"
            ),
        );
        assert_eq!(not_model.status(), StatusCode::BAD_REQUEST);
    }

    #[test]
    fn serves_several_inline_meshes_in_one_answer() {
        let repo = TestRepo::init();
        repo.write(
            "Assets/Room.unity",
            "%YAML 1.1\n--- !u!1 &1\nGameObject:\n  m_Name: A\n",
        )
        .commit_all("one");
        let session = Session::default();
        let id = session.open(repo.path()).unwrap().id;
        let r = respond(
            &session,
            &format!(
                "http://gitgud.localhost/model?repo={id}&path=Assets%2FRoom.unity&at=head&meshes=7,-8"
            ),
        );
        assert_eq!(r.status(), StatusCode::OK);
        let b = r.body();
        let mut at = 0;
        let mut entries = Vec::new();
        while at < b.len() {
            let ok = b[at];
            at += 1;
            let mut field = || {
                let len = u32::from_le_bytes(b[at..at + 4].try_into().unwrap()) as usize;
                at += 4 + len;
                String::from_utf8_lossy(&b[at - len..at]).into_owned()
            };
            let mesh = field();
            entries.push((ok, mesh, field()));
        }
        assert_eq!(
            entries,
            [
                (
                    0,
                    "7".to_owned(),
                    "Assets/Room.unity has no Mesh &7".to_owned()
                ),
                (
                    0,
                    "-8".to_owned(),
                    "Assets/Room.unity has no Mesh &-8".to_owned()
                ),
            ]
        );
        let gone = respond(
            &session,
            &format!(
                "http://gitgud.localhost/model?repo={id}&path=Assets%2FGone.unity&at=head&meshes=7"
            ),
        );
        assert_eq!(gone.status(), StatusCode::NOT_FOUND);
    }

    #[test]
    fn decodes_paths_and_reports_what_is_wrong() {
        let repo = TestRepo::init();
        std::fs::create_dir_all(repo.path().join("a b")).unwrap();
        std::fs::write(repo.path().join("a b/t&1.png"), b"\x89PNG\0x").unwrap();
        let session = Session::default();
        let id = session.open(repo.path()).unwrap().id;

        let ok = respond(
            &session,
            &format!("http://gitgud.localhost/image?repo={id}&path=a%20b%2Ft%261.png&at=worktree"),
        );
        assert_eq!(ok.body(), b"\x89PNG\0x");

        let cases = [
            (
                format!("repo={id}&path=t.png&at=head"),
                StatusCode::NOT_FOUND,
            ),
            (
                format!("repo={id}&path=a.txt&at=head"),
                StatusCode::BAD_REQUEST,
            ),
            (
                format!("repo={id}&path=t.png&at=later"),
                StatusCode::BAD_REQUEST,
            ),
            (
                format!("repo={id}&path=t.png&at=commit"),
                StatusCode::BAD_REQUEST,
            ),
            (
                "repo=99&path=t.png&at=head".to_owned(),
                StatusCode::NOT_FOUND,
            ),
            ("path=t.png&at=head".to_owned(), StatusCode::BAD_REQUEST),
        ];
        for (query, status) in cases {
            let r = respond(&session, &format!("http://gitgud.localhost/image?{query}"));
            assert_eq!(r.status(), status, "{query}: {}", body(&r));
            assert_eq!(r.headers()[header::ACCESS_CONTROL_ALLOW_ORIGIN], "*");
        }
    }

    #[test]
    fn shrinks_textures_on_request() {
        let repo = TestRepo::init();
        let img = image::RgbaImage::new(3000, 1500);
        let mut png = std::io::Cursor::new(Vec::new());
        img.write_to(&mut png, image::ImageFormat::Png).unwrap();
        std::fs::write(repo.path().join("big.png"), png.get_ref()).unwrap();
        repo.commit_all("one");
        let session = Session::default();
        let id = session.open(repo.path()).unwrap().id;
        let get = |q: &str| {
            respond(
                &session,
                &format!("http://gitgud.localhost/image?repo={id}&path=big.png&at=head{q}"),
            )
        };
        let full = get("");
        assert_eq!(full.body(), png.get_ref());
        let small = get("&max=1024");
        assert_eq!(small.headers()[header::CONTENT_TYPE], "image/png");
        let back = image::load_from_memory(small.body()).unwrap();
        assert_eq!((back.width(), back.height()), (1024, 512));

        let bc = get("&max=1024&gpu=bc");
        assert_eq!(bc.headers()[header::CONTENT_TYPE], "image/vnd-ms.dds");
        assert_eq!(&bc.body()[0..4], b"DDS ");
        let ignored = get("&gpu=bc");
        assert_eq!(
            ignored.body(),
            png.get_ref(),
            "only 3D previews (max) are compressed"
        );
    }

    #[test]
    fn serves_a_sound_decoded_to_wav() {
        let repo = TestRepo::init();
        let wav = gitgud_core::audio::sine_wav(22_050, 1, 0.5, 0.5);
        std::fs::write(repo.path().join("horn.wav"), &wav).unwrap();
        repo.write("notes.wav", "not audio").commit_all("one");
        let session = Session::default();
        let id = session.open(repo.path()).unwrap().id;
        let r = respond(
            &session,
            &format!("http://gitgud.localhost/audio?repo={id}&path=horn.wav&at=head"),
        );
        assert_eq!(r.status(), StatusCode::OK);
        assert_eq!(r.headers()[header::CONTENT_TYPE], "audio/wav");
        assert_eq!(r.body(), &wav);
        let bad = respond(
            &session,
            &format!("http://gitgud.localhost/audio?repo={id}&path=notes.wav&at=head"),
        );
        assert_eq!(bad.status(), StatusCode::NOT_FOUND);
        assert!(body(&bad).contains("not a sound file"));
    }
}
