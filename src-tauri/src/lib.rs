use gitgud_core::git::{self, GitInfo};
use gitgud_core::graph::Row;
use gitgud_core::repo::{Error, Status};
use gitgud_core::session::{Opened, Session};
use tauri::{Builder, Runtime, State};

#[tauri::command]
async fn git_info() -> Result<GitInfo, git::Error> {
    git::locate()
}

/// The repository to open at start-up, from `GITGUD_OPEN`.
#[tauri::command]
async fn launch_path() -> Option<String> {
    std::env::var("GITGUD_OPEN").ok().filter(|p| !p.is_empty())
}

#[tauri::command]
async fn open_repo(session: State<'_, Session>, path: String) -> Result<Opened, Error> {
    session.open(path)
}

#[tauri::command]
async fn close_repo(session: State<'_, Session>, id: u32) -> Result<(), Error> {
    session.close(id)
}

#[tauri::command]
async fn repo_status(session: State<'_, Session>, id: u32) -> Result<Status, Error> {
    session.get(id)?.repo.status()
}

#[tauri::command]
async fn graph_rows(
    session: State<'_, Session>,
    id: u32,
    start: usize,
    count: usize,
) -> Result<Vec<Row>, Error> {
    let open = session.get(id)?;
    open.graph.rows(&open.repo, start, count)
}

fn app<R: Runtime>(builder: Builder<R>) -> Builder<R> {
    builder
        .manage(Session::default())
        .invoke_handler(tauri::generate_handler![
            git_info,
            launch_path,
            open_repo,
            close_repo,
            repo_status,
            graph_rows
        ])
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    app(Builder::default())
        .plugin(tauri_plugin_dialog::init())
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[cfg(test)]
mod tests {
    use super::*;
    use gitgud_core::testing::TestRepo;
    use serde_json::{Value, json};
    use tauri::ipc::{CallbackFn, InvokeBody};
    use tauri::test::{
        INVOKE_KEY, MockRuntime, get_ipc_response, mock_builder, mock_context, noop_assets,
    };
    use tauri::webview::{InvokeRequest, WebviewWindow};

    /// Invokes `cmd` through the real IPC path, with JSON args as the webview sends them.
    struct Ipc {
        _app: tauri::App<MockRuntime>,
        webview: WebviewWindow<MockRuntime>,
    }

    impl Ipc {
        fn new() -> Self {
            let app = app(mock_builder())
                .build(mock_context(noop_assets()))
                .unwrap();
            let webview = tauri::WebviewWindowBuilder::new(&app, "main", Default::default())
                .build()
                .unwrap();
            Self { _app: app, webview }
        }

        fn call(&self, cmd: &str, args: Value) -> Result<Value, Value> {
            let request = InvokeRequest {
                cmd: cmd.into(),
                callback: CallbackFn(0),
                error: CallbackFn(1),
                url: "http://tauri.localhost".parse().unwrap(),
                body: InvokeBody::Json(args),
                headers: Default::default(),
                invoke_key: INVOKE_KEY.into(),
            };
            get_ipc_response(&self.webview, request).map(|body| body.deserialize().unwrap())
        }
    }

    fn history() -> TestRepo {
        let repo = TestRepo::init();
        repo.write("a.txt", "a").commit_all("first");
        repo.write("a.txt", "b").commit_all("second");
        repo.git(&["tag", "v1"]);
        repo
    }

    fn open(ipc: &Ipc, repo: &TestRepo) -> Value {
        ipc.call("open_repo", json!({ "path": repo.path() }))
            .unwrap()
    }

    #[test]
    fn git_info_returns_path_and_dotted_version() {
        let info = Ipc::new().call("git_info", json!({})).unwrap();
        assert!(info["path"].as_str().unwrap().contains("git"));
        assert_eq!(info["version"].as_str().unwrap().split('.').count(), 3);
    }

    #[test]
    fn open_repo_returns_id_root_and_row_count() {
        let ipc = Ipc::new();
        let opened = open(&ipc, &history());
        assert_eq!(opened.as_object().unwrap().len(), 3);
        assert!(opened["id"].is_u64());
        assert!(opened["root"].is_string());
        assert_eq!(opened["rows"], 2);
    }

    #[test]
    fn open_repo_rejects_a_plain_folder_with_a_message() {
        let dir = tempfile::tempdir().unwrap();
        let err = Ipc::new()
            .call("open_repo", json!({ "path": dir.path() }))
            .unwrap_err();
        assert!(
            err.as_str()
                .unwrap()
                .ends_with("is not inside a git repository"),
            "{err}"
        );
    }

    #[test]
    fn repo_status_by_id() {
        let ipc = Ipc::new();
        let repo = history();
        repo.write("a.txt", "dirty").write("new.txt", "n");
        let id = open(&ipc, &repo)["id"].clone();

        let status = ipc.call("repo_status", json!({ "id": id })).unwrap();
        assert_eq!(status["head"], json!({ "kind": "branch", "name": "main" }));
        assert_eq!(
            status["entries"],
            json!([
                { "path": "a.txt", "kind": "tracked", "index": null, "worktree": "modified" },
                { "path": "new.txt", "kind": "untracked", "index": null, "worktree": null },
            ])
        );
    }

    #[test]
    fn graph_rows_shape() {
        let ipc = Ipc::new();
        let repo = history();
        let id = open(&ipc, &repo)["id"].clone();

        let rows = ipc
            .call("graph_rows", json!({ "id": id, "start": 0, "count": 10 }))
            .unwrap();
        let rows = rows.as_array().unwrap();
        assert_eq!(rows.len(), 2);
        let head = repo.git(&["rev-parse", "HEAD"]);
        assert_eq!(
            rows[0],
            json!({
                "id": head.trim(),
                "shortId": &head.trim()[..7],
                "subject": "second",
                "author": "gitgud tests",
                "time": rows[0]["time"],
                "labels": [
                    { "kind": "branch", "name": "main", "current": true },
                    { "kind": "tag", "name": "v1" },
                ],
                "wip": false,
                "layout": {
                    "node": 0,
                    "segments": [{ "from": 0, "to": 0, "part": "bottom" }],
                    "overflow": false,
                },
            })
        );
        assert!(rows[0]["time"].as_i64().unwrap() > 1_700_000_000);
    }

    #[test]
    fn closed_and_unknown_ids_fail_with_a_message() {
        let ipc = Ipc::new();
        let id = open(&ipc, &history())["id"].clone();
        ipc.call("close_repo", json!({ "id": id })).unwrap();

        let err = ipc
            .call("graph_rows", json!({ "id": id, "start": 0, "count": 1 }))
            .unwrap_err();
        assert_eq!(err, format!("repository {id} is not open"));
        assert!(ipc.call("repo_status", json!({ "id": 4242 })).is_err());
    }

    #[test]
    fn launch_path_is_null_without_gitgud_open() {
        assert_eq!(
            Ipc::new().call("launch_path", json!({})).unwrap(),
            Value::Null
        );
    }
}
