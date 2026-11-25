use std::path::PathBuf;

use serde::{Deserialize, Serialize};

/// Which git gitgud runs, as picked in Settings and shared with the CLI / MCP server.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(tag = "mode", rename_all = "camelCase")]
pub enum GitChoice {
    /// The system git when it is new enough, else the embedded one.
    #[default]
    Auto,
    System,
    Embedded,
    Custom {
        path: PathBuf,
    },
}

const FILE: &str = "git.json";
const APP_ID: &str = "com.happyrave.gitgud";

/// The settings file: `GITGUD_GIT_SETTINGS`, else `%APPDATA%\com.happyrave.gitgud\git.json`
/// (the app's data folder, next to `library.json`).
#[must_use]
pub fn settings_path() -> Option<PathBuf> {
    if let Some(p) = std::env::var_os("GITGUD_GIT_SETTINGS").filter(|v| !v.is_empty()) {
        return Some(PathBuf::from(p));
    }
    let base = std::env::var_os("APPDATA")
        .or_else(|| std::env::var_os("XDG_CONFIG_HOME"))
        .filter(|v| !v.is_empty())?;
    Some(PathBuf::from(base).join(APP_ID).join(FILE))
}

impl GitChoice {
    /// The saved choice; `Auto` when nothing is saved yet. A file that can't be read is
    /// reported on stderr and treated as `Auto`, so a broken file never locks git out.
    #[must_use]
    pub fn load() -> Self {
        settings_path().map_or_else(Self::default, |p| Self::load_from(&p))
    }

    /// As [`GitChoice::load`], from `path`.
    #[must_use]
    pub fn load_from(path: &std::path::Path) -> Self {
        let text = match std::fs::read_to_string(path) {
            Ok(text) => text,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Self::default(),
            Err(e) => {
                eprintln!("gitgud: could not read {}: {e}", path.display());
                return Self::default();
            }
        };
        serde_json::from_str(&text).unwrap_or_else(|e| {
            eprintln!("gitgud: ignoring invalid {}: {e}", path.display());
            Self::default()
        })
    }

    /// Writes the choice to the settings file.
    ///
    /// # Errors
    /// When there is no settings folder or the file can't be written.
    pub fn save(&self) -> std::io::Result<()> {
        let path = settings_path()
            .ok_or_else(|| std::io::Error::other("no APPDATA folder to save the git choice in"))?;
        self.save_to(&path)
    }

    /// As [`GitChoice::save`], to `path`.
    ///
    /// # Errors
    /// When the file can't be written.
    pub fn save_to(&self, path: &std::path::Path) -> std::io::Result<()> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let json = serde_json::to_string_pretty(self).map_err(std::io::Error::other)?;
        std::fs::write(path, json)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_every_mode() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("sub").join("git.json");
        for choice in [
            GitChoice::Auto,
            GitChoice::System,
            GitChoice::Embedded,
            GitChoice::Custom {
                path: PathBuf::from(r"D:\tools\git\cmd\git.exe"),
            },
        ] {
            choice.save_to(&path).unwrap();
            assert_eq!(GitChoice::load_from(&path), choice);
        }
    }

    #[test]
    fn missing_or_broken_file_is_auto() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("git.json");
        assert_eq!(GitChoice::load_from(&path), GitChoice::Auto);
        std::fs::write(&path, "{ not json").unwrap();
        assert_eq!(GitChoice::load_from(&path), GitChoice::Auto);
    }

    #[test]
    fn json_shape_is_tagged() {
        let json = serde_json::to_string(&GitChoice::Custom {
            path: PathBuf::from("g"),
        })
        .unwrap();
        assert_eq!(json, r#"{"mode":"custom","path":"g"}"#);
    }
}
