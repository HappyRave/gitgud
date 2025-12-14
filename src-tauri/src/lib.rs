mod git_operations;

use git_operations::*;

#[tauri::command]
fn get_repo_info_cmd(path: String) -> Result<RepoInfo, String> {
    get_repo_info(&path)
}

#[tauri::command]
fn get_repo_status_cmd(path: String) -> Result<RepoStatus, String> {
    get_repo_status(&path)
}

#[tauri::command]
fn stage_file_cmd(path: String, file_path: String) -> Result<(), String> {
    stage_file(&path, &file_path)
}

#[tauri::command]
fn unstage_file_cmd(path: String, file_path: String) -> Result<(), String> {
    unstage_file(&path, &file_path)
}

#[tauri::command]
fn commit_cmd(path: String, message: String) -> Result<String, String> {
    commit(&path, &message)
}

#[tauri::command]
fn get_commits_cmd(path: String, limit: usize) -> Result<Vec<CommitInfo>, String> {
    get_commits(&path, limit)
}

#[tauri::command]
fn get_git_version_cmd() -> Result<GitVersionInfo, String> {
    get_git_version()
}

#[tauri::command]
fn pull_cmd(path: String) -> Result<String, String> {
    pull(&path)
}

#[tauri::command]
fn push_cmd(path: String) -> Result<String, String> {
    push(&path)
}

#[tauri::command]
fn push_with_credentials_cmd(
    path: String,
    username: Option<String>,
    password: Option<String>,
) -> Result<String, String> {
    push_with_credentials(&path, username, password)
}

#[tauri::command]
fn get_branches_cmd(path: String) -> Result<Vec<BranchInfo>, String> {
    get_branches(&path)
}

#[tauri::command]
fn checkout_branch_cmd(path: String, branch_name: String) -> Result<(), String> {
    checkout_branch(&path, &branch_name)
}

#[tauri::command]
fn clone_repository_cmd(url: String, path: String) -> Result<String, String> {
    clone_repository(&url, &path)
}

#[tauri::command]
fn init_repository_cmd(path: String) -> Result<String, String> {
    init_repository(&path)
}

#[tauri::command]
fn add_remote_cmd(path: String, name: String, url: String) -> Result<(), String> {
    add_remote(&path, &name, &url)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_store::Builder::new().build())
        .setup(|app| {
            if cfg!(debug_assertions) {
                app.handle().plugin(
                    tauri_plugin_log::Builder::default()
                        .level(log::LevelFilter::Info)
                        .build(),
                )?;
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_repo_info_cmd,
            get_repo_status_cmd,
            stage_file_cmd,
            unstage_file_cmd,
            commit_cmd,
            get_commits_cmd,
            pull_cmd,
            push_cmd,
            get_git_version_cmd,
            push_with_credentials_cmd,
            get_branches_cmd,
            checkout_branch_cmd,
            clone_repository_cmd,
            init_repository_cmd,
            add_remote_cmd
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
