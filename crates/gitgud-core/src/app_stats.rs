use std::collections::HashMap;

use serde::Serialize;
use sysinfo::{Pid, ProcessRefreshKind, ProcessesToUpdate, System};

/// gitgud's CPU and memory, its whole process tree counted.
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppStats {
    /// Share of the whole machine, 0–100 (100 = every core busy).
    pub cpu: f32,
    /// Working set of gitgud's own process.
    pub app_bytes: u64,
    /// Working set of the WebView2 processes drawing the window.
    pub view_bytes: u64,
    /// Working set of everything else gitgud started: git and its helpers.
    pub git_bytes: u64,
}

/// One process as [`summarize`] needs it.
#[derive(Debug, Clone)]
pub struct Proc {
    pub pid: u32,
    pub parent: Option<u32>,
    pub name: String,
    /// Percent of one core, as the OS reports it.
    pub cpu: f32,
    pub bytes: u64,
}

/// Samples [`AppStats`] for a process tree; keep one and call [`ProcessMonitor::sample`]
/// at an interval, since CPU use is measured between two samples.
pub struct ProcessMonitor {
    sys: System,
    root: u32,
    cores: f32,
}

impl ProcessMonitor {
    /// Watches the tree under `root` (gitgud's own process in the app).
    ///
    /// # Errors
    /// When the OS can't be queried for processes.
    pub fn new(root: u32) -> Result<Self, String> {
        let sys = System::new().map_err(|e| format!("cannot read processes: {e:?}"))?;
        let cores = std::thread::available_parallelism().map_or(1, std::num::NonZero::get);
        #[allow(clippy::cast_precision_loss)]
        let cores = cores as f32;
        Ok(Self { sys, root, cores })
    }

    /// Refreshes the process table and sums the tree. The first sample's CPU is 0.
    pub fn sample(&mut self) -> AppStats {
        self.sys.refresh_processes_specifics(
            ProcessesToUpdate::All,
            true,
            ProcessRefreshKind::nothing().with_cpu().with_memory(),
        );
        let procs: Vec<Proc> = self
            .sys
            .processes()
            .values()
            .map(|p| Proc {
                pid: p.pid().as_u32(),
                parent: p.parent().map(Pid::as_u32),
                name: p.name().to_string_lossy().into_owned(),
                cpu: p.cpu_usage(),
                bytes: p.memory(),
            })
            .collect();
        summarize(&procs, self.root, self.cores)
    }
}

/// Sums `root` and its descendants: WebView2 processes count as the view, `root` as the
/// app, anything else under it (git, ssh, sh…) as git.
#[must_use]
pub fn summarize(procs: &[Proc], root: u32, cores: f32) -> AppStats {
    let mut children: HashMap<u32, Vec<&Proc>> = HashMap::new();
    for p in procs {
        if let Some(parent) = p.parent.filter(|&parent| parent != p.pid) {
            children.entry(parent).or_default().push(p);
        }
    }
    let mut stats = AppStats::default();
    let mut cpu = 0.0;
    let mut todo: Vec<&Proc> = procs.iter().filter(|p| p.pid == root).collect();
    // A reused pid can name a dead parent's slot and close a loop.
    let mut seen = std::collections::HashSet::new();
    while let Some(p) = todo.pop() {
        if !seen.insert(p.pid) {
            continue;
        }
        cpu += p.cpu;
        if p.pid == root {
            stats.app_bytes += p.bytes;
        } else if p.name.eq_ignore_ascii_case("msedgewebview2.exe") {
            stats.view_bytes += p.bytes;
        } else {
            stats.git_bytes += p.bytes;
        }
        if let Some(kids) = children.get(&p.pid) {
            todo.extend(kids);
        }
    }
    stats.cpu = (cpu / cores.max(1.0)).clamp(0.0, 100.0);
    stats
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(pid: u32, parent: Option<u32>, name: &str, cpu: f32, bytes: u64) -> Proc {
        Proc {
            pid,
            parent,
            name: name.into(),
            cpu,
            bytes,
        }
    }

    #[test]
    fn sums_the_tree_and_splits_app_view_and_git() {
        let procs = [
            p(1, None, "explorer.exe", 50.0, 9_000),
            p(10, Some(1), "gitgud.exe", 20.0, 100),
            p(11, Some(10), "msedgewebview2.exe", 30.0, 200),
            p(12, Some(11), "msedgewebview2.exe", 10.0, 50),
            p(13, Some(10), "git.exe", 15.0, 30),
            p(14, Some(13), "git-remote-https.exe", 5.0, 7),
            p(20, Some(1), "msedgewebview2.exe", 99.0, 9_999),
        ];
        let s = summarize(&procs, 10, 4.0);
        assert!((s.cpu - 20.0).abs() < 1e-4, "{}", s.cpu);
        assert_eq!((s.app_bytes, s.view_bytes, s.git_bytes), (100, 250, 37));
    }

    #[test]
    fn survives_a_reused_pid_that_closes_a_loop() {
        let procs = [
            p(5, Some(9), "gitgud.exe", 1.0, 10),
            p(9, Some(5), "git.exe", 1.0, 20),
        ];
        let stats = summarize(&procs, 5, 1.0);
        assert_eq!((stats.app_bytes, stats.git_bytes), (10, 20));
    }

    #[test]
    fn caps_cpu_and_survives_a_missing_root_or_a_self_parent() {
        let procs = [p(5, Some(5), "gitgud.exe", 900.0, 1)];
        assert_eq!(summarize(&procs, 5, 8.0).cpu, 100.0);
        assert_eq!(summarize(&procs, 6, 8.0), AppStats::default());
    }

    #[test]
    fn samples_this_process_and_a_child() {
        let mut child = std::process::Command::new("ping")
            .args([if cfg!(windows) { "-n" } else { "-c" }, "30", "127.0.0.1"])
            .stdout(std::process::Stdio::null())
            .spawn()
            .unwrap();
        let mut monitor = ProcessMonitor::new(std::process::id()).unwrap();
        monitor.sample();
        std::thread::sleep(sysinfo::MINIMUM_CPU_UPDATE_INTERVAL);
        let s = monitor.sample();
        child.kill().unwrap();
        child.wait().unwrap();
        assert!(s.app_bytes > 0, "{s:?}");
        assert!(s.git_bytes > 0, "the child counts: {s:?}");
        assert!((0.0..=100.0).contains(&s.cpu), "{s:?}");
    }
}
