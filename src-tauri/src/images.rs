use gitgud_core::image::{Version, image_type};
use gitgud_core::repo::Error;
use gitgud_core::session::Session;
use tauri::Url;
use tauri::http::{Response, StatusCode, header};

/// Answers `gitgud://localhost/image?repo=1&path=a.png&at=index` (on Windows
/// `http://gitgud.localhost/image?...`) with the image bytes, so the webview can
/// show a version of an image without base64 over IPC. `at` is `head`,
/// `index`, `worktree`, `yours`, `theirs`, or `commit` / `parent` with `commit=<id>`.
pub fn respond(session: &Session, uri: &str) -> Response<Vec<u8>> {
    match read(session, uri) {
        Ok((bytes, mime)) => Response::builder()
            .header(header::CONTENT_TYPE, mime)
            .header(header::CACHE_CONTROL, "no-store")
            .header(header::ACCESS_CONTROL_ALLOW_ORIGIN, "*")
            .body(bytes),
        Err((status, message)) => Response::builder()
            .status(status)
            .header(header::CONTENT_TYPE, "text/plain; charset=utf-8")
            .header(header::ACCESS_CONTROL_ALLOW_ORIGIN, "*")
            .body(message.into_bytes()),
    }
    .expect("static headers are valid")
}

fn read(session: &Session, uri: &str) -> Result<(Vec<u8>, &'static str), (StatusCode, String)> {
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
    let mime = image_type(&path).ok_or_else(|| bad("not an image"))?;
    let auto_fetch = param("autofetch").as_deref() != Some("0");
    let bytes = session
        .image_bytes(repo, &path, &version, auto_fetch)
        .map_err(fail)?;
    Ok((bytes, mime))
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
}
