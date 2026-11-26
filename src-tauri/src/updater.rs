use std::path::Path;
use std::sync::Mutex;

use serde::Serialize;
use tauri::{AppHandle, Emitter, Runtime, State};
use tauri_plugin_updater::{Update, UpdaterExt};

pub const UPDATE_PROGRESS: &str = "update-progress";
const RELEASES: &str = "https://github.com/HappyRave/gitgud/releases/tag/v";

/// The update found by the last check, kept for `update_install`.
#[derive(Default)]
pub struct PendingUpdate(Mutex<Option<Update>>);

#[derive(Serialize, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct UpdateInfo {
    version: String,
    current: String,
    notes: Option<String>,
    /// A portable copy can't run the installer; it gets the release page instead.
    portable: bool,
    page: String,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct Progress {
    downloaded: u64,
    total: Option<u64>,
}

/// An installed gitgud has the NSIS uninstaller next to it; a portable copy doesn't.
pub fn is_portable(exe_dir: &Path) -> bool {
    !exe_dir.join("uninstall.exe").is_file()
}

fn info(update: &Update, portable: bool) -> UpdateInfo {
    UpdateInfo {
        version: update.version.clone(),
        current: update.current_version.clone(),
        notes: update.body.clone().filter(|b| !b.trim().is_empty()),
        portable,
        page: format!("{RELEASES}{}", update.version),
    }
}

/// The newer release, if any. Debug builds only check when asked (`manual`), so dev runs and
/// e2e tests stay off the network.
#[tauri::command]
pub async fn update_check<R: Runtime>(
    app: AppHandle<R>,
    pending: State<'_, PendingUpdate>,
    manual: bool,
) -> Result<Option<UpdateInfo>, String> {
    if cfg!(debug_assertions) && !manual {
        return Ok(None);
    }
    let update = app
        .updater_builder()
        .on_before_exit(gitgud_core::preview::clean_open_copies)
        .build()
        .map_err(|e| e.to_string())?
        .check()
        .await
        .map_err(|e| format!("cannot check for updates: {e}"))?;
    let exe_dir = std::env::current_exe()
        .map_err(|e| e.to_string())?
        .parent()
        .map(Path::to_path_buf)
        .ok_or("gitgud's own folder is unknown")?;
    let found = update.as_ref().map(|u| info(u, is_portable(&exe_dir)));
    *pending.0.lock().unwrap() = update;
    Ok(found)
}

/// Downloads and runs the installer, which replaces gitgud and starts it again. On Windows the
/// app exits here and never returns.
#[tauri::command]
pub async fn update_install<R: Runtime>(
    app: AppHandle<R>,
    pending: State<'_, PendingUpdate>,
) -> Result<(), String> {
    let update = pending
        .0
        .lock()
        .unwrap()
        .take()
        .ok_or("no update to install; check again")?;
    let mut downloaded = 0u64;
    update
        .download_and_install(
            |chunk, total| {
                downloaded += chunk as u64;
                if let Err(e) = app.emit(UPDATE_PROGRESS, Progress { downloaded, total }) {
                    eprintln!("gitgud: cannot emit {UPDATE_PROGRESS}: {e}");
                }
            },
            || {},
        )
        .await
        .map_err(|e| format!("cannot install the update: {e}"))?;
    app.restart()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn portable_without_the_uninstaller() {
        let dir = tempfile::tempdir().unwrap();
        assert!(is_portable(dir.path()));
        std::fs::write(dir.path().join("uninstall.exe"), b"").unwrap();
        assert!(!is_portable(dir.path()));
    }
}
