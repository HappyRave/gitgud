use gitgud_core::git::{self, GitInfo};
use gitgud_core::repo::{self, Status};

#[tauri::command]
async fn git_info() -> Result<GitInfo, git::Error> {
    git::locate()
}

#[tauri::command]
async fn repo_status(path: String) -> Result<Status, repo::Error> {
    repo::open(path)?.status()
}

/// The repository to open at start-up, from `GITGUD_OPEN`.
#[tauri::command]
async fn launch_path() -> Option<String> {
    std::env::var("GITGUD_OPEN").ok().filter(|p| !p.is_empty())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![git_info, repo_status, launch_path])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[cfg(test)]
mod tests {
    use super::*;
    use gitgud_core::testing::TestRepo;
    use serde_json::json;
    use tauri::async_runtime::block_on;

    #[test]
    fn git_info_serializes_to_the_shape_api_ts_expects() {
        let json = serde_json::to_value(block_on(git_info()).unwrap()).unwrap();
        let obj = json.as_object().unwrap();
        assert_eq!(obj.len(), 2);
        assert!(obj["path"].as_str().unwrap().contains("git"));
        let version = obj["version"].as_str().unwrap();
        assert_eq!(version.split('.').count(), 3, "{version}");
    }

    #[test]
    fn errors_serialize_to_their_message() {
        let json = serde_json::to_value(git::Error::NotFound).unwrap();
        assert_eq!(json, "git was not found on PATH");
    }

    #[test]
    fn repo_status_serializes_to_the_shape_api_ts_expects() {
        let repo = TestRepo::init();
        repo.write("a.txt", "a\n").commit_all("init");
        repo.write("a.txt", "b\n").write("new.txt", "n\n");
        let path = repo.path().to_string_lossy().into_owned();

        let json = serde_json::to_value(block_on(repo_status(path)).unwrap()).unwrap();
        let obj = json.as_object().unwrap();
        assert_eq!(obj.len(), 3);
        assert!(obj["root"].is_string());
        assert_eq!(obj["head"], json!({ "kind": "branch", "name": "main" }));
        assert_eq!(
            obj["entries"],
            json!([
                { "path": "a.txt", "kind": "tracked", "index": null, "worktree": "modified" },
                { "path": "new.txt", "kind": "untracked", "index": null, "worktree": null },
            ])
        );
    }

    #[test]
    fn repo_status_rejects_a_plain_folder_with_a_message() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().to_string_lossy().into_owned();
        let err = block_on(repo_status(path.clone())).unwrap_err();
        let json = serde_json::to_value(err).unwrap();
        assert_eq!(json, format!("{path} is not inside a git repository"));
    }
}
