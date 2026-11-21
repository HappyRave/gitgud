use gitgud_core::git::{self, GitInfo};

#[tauri::command]
fn git_info() -> Result<GitInfo, git::Error> {
    git::locate()
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![git_info])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn git_info_serializes_to_the_shape_api_ts_expects() {
        let json = serde_json::to_value(git_info().unwrap()).unwrap();
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
}
