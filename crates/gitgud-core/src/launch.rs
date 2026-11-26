use std::path::{Component, Path, PathBuf};
use std::process::Command;

use serde::{Deserialize, Serialize};

use crate::repo::Error;

/// The terminal "Open terminal here" starts (Settings › Tools).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Terminal {
    /// Windows Terminal's default profile; PowerShell when `wt` isn't installed.
    WindowsTerminal,
    PowerShell,
    GitBash,
    Cmd,
}

/// The editor "Open in editor" starts (Settings › Tools).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum Editor {
    VsCode,
    Rider,
    VisualStudio,
    /// Any program, given the file (or the folder) as its one argument.
    Custom {
        program: String,
    },
}

/// One process to start: what the launchers below decide, before anything runs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Launch {
    pub program: PathBuf,
    pub args: Vec<String>,
    /// Working directory of the new process.
    pub cwd: PathBuf,
    /// A console program that needs its own window (PowerShell, cmd).
    pub new_console: bool,
}

/// Which terminals and editors this machine has, for the Settings lists.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LaunchTools {
    pub windows_terminal: bool,
    pub git_bash: bool,
    pub vs_code: bool,
    pub rider: bool,
    pub visual_studio: bool,
}

/// Finds an executable on `PATH` by file name.
pub type OnPath = Box<dyn Fn(&str) -> Option<PathBuf>>;

/// Where tools are found; [`Finder::system`] looks on this machine, tests pass their own.
pub struct Finder {
    pub on_path: OnPath,
    pub git_bash: Option<PathBuf>,
    /// `Code.exe`, started directly: `code.cmd` through `cmd /C` mangles quoted paths.
    pub vs_code: Option<PathBuf>,
    pub rider: Option<PathBuf>,
    pub visual_studio: Option<PathBuf>,
}

impl Finder {
    #[must_use]
    pub fn system() -> Self {
        Self {
            on_path: Box::new(which),
            git_bash: find_git_bash(),
            vs_code: find_vs_code(),
            rider: find_rider(),
            visual_studio: find_visual_studio(),
        }
    }

    #[must_use]
    pub fn tools(&self) -> LaunchTools {
        LaunchTools {
            windows_terminal: (self.on_path)("wt.exe").is_some(),
            git_bash: self.git_bash.is_some(),
            vs_code: self.vs_code.is_some(),
            rider: self.rider.is_some(),
            visual_studio: self.visual_studio.is_some(),
        }
    }
}

/// `root` joined with repo-relative `path`, refused when it would leave `root`.
///
/// # Errors
/// [`Error::Refused`] for an absolute path or one climbing out with `..`.
pub fn inside(root: &Path, path: Option<&str>) -> Result<PathBuf, Error> {
    let Some(path) = path.filter(|p| !p.is_empty()) else {
        return Ok(root.to_path_buf());
    };
    let rel = Path::new(path);
    if rel
        .components()
        .any(|c| !matches!(c, Component::Normal(_) | Component::CurDir))
    {
        return Err(Error::Refused(format!(
            "{path} isn't inside the repository"
        )));
    }
    Ok(root.join(rel))
}

/// The nearest folder that exists at or above `target` (a deleted file shows its folder).
fn existing_dir(root: &Path, target: &Path) -> PathBuf {
    let mut at = target.to_path_buf();
    loop {
        if at.is_dir() {
            return at;
        }
        if !at.pop() || !at.starts_with(root) {
            return root.to_path_buf();
        }
    }
}

/// Explorer with `target` selected, or its nearest existing folder opened.
#[must_use]
pub fn reveal(root: &Path, target: &Path) -> Launch {
    let args = if target.is_file() {
        vec![format!("/select,{}", windows_path(target))]
    } else {
        vec![windows_path(&existing_dir(root, target))]
    };
    Launch {
        program: "explorer.exe".into(),
        args,
        cwd: root.to_path_buf(),
        new_console: false,
    }
}

/// A terminal in `target`'s folder (its own folder when it is one).
///
/// # Errors
/// [`Error::Refused`] when Git Bash is chosen and not installed.
pub fn terminal(
    finder: &Finder,
    terminal: Terminal,
    root: &Path,
    target: &Path,
) -> Result<Launch, Error> {
    let dir = existing_dir(root, target);
    let shown = windows_path(&dir);
    let powershell = || Launch {
        program: "powershell.exe".into(),
        args: vec!["-NoExit".into(), "-NoLogo".into()],
        cwd: dir.clone(),
        new_console: true,
    };
    Ok(match terminal {
        Terminal::WindowsTerminal => match (finder.on_path)("wt.exe") {
            Some(wt) => Launch {
                program: wt,
                args: vec!["-d".into(), shown],
                cwd: dir.clone(),
                new_console: false,
            },
            None => powershell(),
        },
        Terminal::PowerShell => powershell(),
        Terminal::GitBash => Launch {
            program: finder.git_bash.clone().ok_or_else(|| {
                Error::Refused("Git Bash isn't installed (it comes with Git for Windows)".into())
            })?,
            args: vec![format!("--cd={shown}")],
            cwd: dir.clone(),
            new_console: false,
        },
        Terminal::Cmd => Launch {
            program: "cmd.exe".into(),
            args: vec!["/K".into()],
            cwd: dir.clone(),
            new_console: true,
        },
    })
}

/// The editor on `target`: a file opens in the repo's folder (VS Code, Rider) or alone; a
/// folder opens as a folder.
///
/// # Errors
/// [`Error::Refused`] when the editor isn't installed, or `target` is gone.
pub fn editor(
    finder: &Finder,
    editor: &Editor,
    root: &Path,
    target: &Path,
) -> Result<Launch, Error> {
    if !target.exists() {
        return Err(Error::Refused(format!(
            "{} isn't on disk",
            target.display()
        )));
    }
    let file = target.is_file().then(|| windows_path(target));
    let folder = windows_path(root);
    let missing = |name: &str| {
        Error::Refused(format!(
            "{name} isn't installed; pick another editor in Settings › Tools"
        ))
    };
    let launch = |program: PathBuf, args: Vec<String>| Launch {
        program,
        args,
        cwd: root.to_path_buf(),
        new_console: false,
    };
    Ok(match editor {
        Editor::VsCode => {
            let code = finder.vs_code.clone().ok_or_else(|| missing("VS Code"))?;
            let mut args = vec![folder];
            if let Some(file) = file {
                args.extend(["-g".to_owned(), file]);
            }
            launch(code, args)
        }
        Editor::Rider => {
            let rider = finder.rider.clone().ok_or_else(|| missing("Rider"))?;
            launch(rider, vec![file.unwrap_or(folder)])
        }
        Editor::VisualStudio => {
            let vs = finder
                .visual_studio
                .clone()
                .ok_or_else(|| missing("Visual Studio"))?;
            match file {
                Some(file) => launch(vs, vec!["/Edit".into(), file]),
                None => launch(vs, vec![folder]),
            }
        }
        Editor::Custom { program } => {
            let program = program.trim().trim_matches('"');
            if program.is_empty() {
                return Err(Error::Refused(
                    "set the editor's program in Settings › Tools".into(),
                ));
            }
            launch(program.into(), vec![file.unwrap_or(folder)])
        }
    })
}

/// Starts `launch`, without waiting for it.
///
/// # Errors
/// [`Error::Refused`] with the OS error when it can't start.
pub fn spawn(launch: &Launch) -> Result<(), Error> {
    let mut cmd = Command::new(&launch.program);
    cmd.args(&launch.args).current_dir(&launch.cwd);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NEW_CONSOLE: u32 = 0x0000_0010;
        if launch.new_console {
            cmd.creation_flags(CREATE_NEW_CONSOLE);
        }
    }
    cmd.spawn()
        .map(drop)
        .map_err(|e| Error::Refused(format!("couldn't start {}: {e}", launch.program.display())))
}

fn windows_path(p: &Path) -> String {
    p.to_string_lossy().replace('/', "\\")
}

fn which(name: &str) -> Option<PathBuf> {
    std::env::split_paths(&std::env::var_os("PATH")?)
        .map(|dir| dir.join(name))
        .find(|p| p.is_file())
}

fn find_git_bash() -> Option<PathBuf> {
    let from_path = which("git.exe").and_then(|git| {
        // …\Git\cmd\git.exe or …\Git\bin\git.exe → …\Git\git-bash.exe
        git.parent()?.parent().map(|g| g.join("git-bash.exe"))
    });
    from_path
        .into_iter()
        .chain(program_files().map(|p| p.join("Git").join("git-bash.exe")))
        .find(|p| p.is_file())
}

fn find_vs_code() -> Option<PathBuf> {
    let from_path =
        which("code.cmd").and_then(|cmd| Some(cmd.parent()?.parent()?.join("Code.exe")));
    let local = std::env::var_os("LOCALAPPDATA").map(|l| {
        PathBuf::from(l)
            .join("Programs")
            .join("Microsoft VS Code")
            .join("Code.exe")
    });
    from_path
        .into_iter()
        .chain(local)
        .chain(program_files().map(|p| p.join("Microsoft VS Code").join("Code.exe")))
        .find(|p| p.is_file())
}

fn find_rider() -> Option<PathBuf> {
    if let Some(p) = which("rider64.exe") {
        return Some(p);
    }
    let local = std::env::var_os("LOCALAPPDATA").map(PathBuf::from);
    let roots = program_files()
        .map(|p| p.join("JetBrains"))
        .chain(local.iter().map(|l| l.join("Programs")))
        .chain(
            local
                .iter()
                .map(|l| l.join("JetBrains").join("Toolbox").join("apps")),
        );
    let mut found: Vec<PathBuf> = Vec::new();
    for root in roots {
        let Ok(entries) = std::fs::read_dir(&root) else {
            continue;
        };
        for e in entries.flatten() {
            let name = e.file_name().to_string_lossy().to_lowercase();
            if name.contains("rider") {
                let exe = e.path().join("bin").join("rider64.exe");
                if exe.is_file() {
                    found.push(exe);
                }
            }
        }
    }
    found.sort();
    found.pop()
}

fn find_visual_studio() -> Option<PathBuf> {
    let installer = program_files()
        .map(|p| {
            p.join("Microsoft Visual Studio")
                .join("Installer")
                .join("vswhere.exe")
        })
        .find(|p| p.is_file())?;
    let out = Command::new(installer)
        .args(["-latest", "-property", "productPath"])
        .output()
        .ok()?;
    let path = PathBuf::from(String::from_utf8_lossy(&out.stdout).trim());
    path.is_file().then_some(path)
}

fn program_files() -> impl Iterator<Item = PathBuf> {
    ["ProgramFiles", "ProgramFiles(x86)"]
        .into_iter()
        .filter_map(std::env::var_os)
        .map(PathBuf::from)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn finder(on_path: &'static [&'static str]) -> Finder {
        Finder {
            on_path: Box::new(move |name| {
                on_path
                    .contains(&name)
                    .then(|| PathBuf::from(format!("C:\\bin\\{name}")))
            }),
            git_bash: None,
            vs_code: None,
            rider: None,
            visual_studio: None,
        }
    }

    fn with_code() -> Finder {
        Finder {
            vs_code: Some("C:\\Code\\Code.exe".into()),
            ..finder(&[])
        }
    }

    fn repo() -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("Assets/Scenes")).unwrap();
        std::fs::write(dir.path().join("Assets/Scenes/Lobby.unity"), "x").unwrap();
        dir
    }

    #[test]
    fn paths_stay_inside_the_repository() {
        let root = Path::new("C:/work/repo");
        assert_eq!(inside(root, None).unwrap(), root);
        assert_eq!(inside(root, Some("a/b.cs")).unwrap(), root.join("a/b.cs"));
        for bad in ["../x", "a/../../x", "C:/Windows", "/etc"] {
            assert!(
                matches!(inside(root, Some(bad)), Err(Error::Refused(_))),
                "{bad}"
            );
        }
    }

    #[test]
    fn explorer_selects_a_file_and_opens_the_nearest_folder_of_a_deleted_one() {
        let dir = repo();
        let root = dir.path();
        let file = root.join("Assets/Scenes/Lobby.unity");
        assert_eq!(
            reveal(root, &file).args,
            [format!("/select,{}", windows_path(&file))]
        );
        let gone = root.join("Assets/Scenes/Gone/Old.unity");
        assert_eq!(
            reveal(root, &gone).args,
            [windows_path(&root.join("Assets/Scenes"))]
        );
    }

    #[test]
    fn windows_terminal_opens_in_the_file_s_folder_or_powershell_without_it() {
        let dir = repo();
        let root = dir.path();
        let file = root.join("Assets/Scenes/Lobby.unity");
        let wt = terminal(&finder(&["wt.exe"]), Terminal::WindowsTerminal, root, &file).unwrap();
        assert_eq!(wt.program, PathBuf::from("C:\\bin\\wt.exe"));
        assert_eq!(
            wt.args,
            ["-d".to_owned(), windows_path(&root.join("Assets/Scenes"))]
        );
        let fallback = terminal(&finder(&[]), Terminal::WindowsTerminal, root, &file).unwrap();
        assert_eq!(fallback.program, PathBuf::from("powershell.exe"));
        assert!(fallback.new_console);
        assert_eq!(fallback.cwd, root.join("Assets/Scenes"));
        assert!(matches!(
            terminal(&finder(&[]), Terminal::GitBash, root, root),
            Err(Error::Refused(_))
        ));
    }

    #[test]
    fn editors_open_the_repo_folder_with_the_file() {
        let dir = repo();
        let root = dir.path();
        let file = root.join("Assets/Scenes/Lobby.unity");
        let code = editor(&with_code(), &Editor::VsCode, root, &file).unwrap();
        assert_eq!(code.program, PathBuf::from("C:\\Code\\Code.exe"));
        assert_eq!(
            code.args,
            [windows_path(root), "-g".into(), windows_path(&file)]
        );
        let folder = editor(&with_code(), &Editor::VsCode, root, root).unwrap();
        assert_eq!(
            folder.args,
            [windows_path(root)],
            "a folder opens without -g"
        );
        assert!(matches!(
            editor(&finder(&[]), &Editor::VsCode, root, &file),
            Err(Error::Refused(_))
        ));
        assert!(matches!(
            editor(&with_code(), &Editor::VsCode, root, &root.join("gone.cs")),
            Err(Error::Refused(_))
        ));

        let custom = editor(
            &finder(&[]),
            &Editor::Custom {
                program: "\"C:\\npp\\notepad++.exe\"".into(),
            },
            root,
            &file,
        )
        .unwrap();
        assert_eq!(custom.program, PathBuf::from("C:\\npp\\notepad++.exe"));
        assert_eq!(custom.args, [windows_path(&file)]);
        assert!(matches!(
            editor(
                &finder(&[]),
                &Editor::Custom {
                    program: " ".into()
                },
                root,
                &file
            ),
            Err(Error::Refused(_))
        ));

        let vs = Finder {
            visual_studio: Some("C:\\VS\\devenv.exe".into()),
            ..finder(&[])
        };
        assert_eq!(
            editor(&vs, &Editor::VisualStudio, root, &file)
                .unwrap()
                .args,
            ["/Edit".to_owned(), windows_path(&file)]
        );
    }

    #[test]
    fn tools_report_what_is_installed() {
        let f = Finder {
            git_bash: Some("C:\\Git\\git-bash.exe".into()),
            ..finder(&["wt.exe"])
        };
        assert_eq!(
            f.tools(),
            LaunchTools {
                windows_terminal: true,
                git_bash: true,
                vs_code: false,
                rider: false,
                visual_studio: false
            }
        );
    }

    /// Prints what this machine has: `cargo test -p gitgud-core --lib detects_tools_here -- --ignored --nocapture`.
    #[test]
    #[ignore = "reads this machine's installs"]
    fn detects_tools_here() {
        let f = Finder::system();
        println!("{:?}", f.tools());
        println!(
            "git bash {:?} · code {:?} · rider {:?} · vs {:?}",
            f.git_bash, f.vs_code, f.rider, f.visual_studio
        );
    }
}
