use notify::{RecursiveMode, Watcher};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{RecvTimeoutError, channel};
use std::time::{Duration, Instant};

// usage: watchbench <repo> latency
//        watchbench <repo> storm <quiet_ms>   (prints stats once no event arrived for quiet_ms; prints READY when armed)
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();
    let root = PathBuf::from(&args[1]);
    let mode = args[2].as_str();
    let (tx, rx) = channel();
    let mut w = notify::recommended_watcher(tx)?;
    let t = Instant::now();
    w.watch(&root, RecursiveMode::Recursive)?;
    println!("watch setup: {:.1} ms", t.elapsed().as_secs_f64() * 1000.0);

    if mode == "latency" {
        let probe = root.join("gitgud-probe.txt");
        let mut lat = Vec::new();
        for i in 0..30 {
            while rx.try_recv().is_ok() {}
            let t = Instant::now();
            std::fs::write(&probe, format!("{i}"))?;
            loop {
                let ev = rx.recv_timeout(Duration::from_secs(2))??;
                if ev.paths.iter().any(|p| p == &probe) {
                    lat.push(t.elapsed().as_secs_f64() * 1000.0);
                    break;
                }
            }
            std::thread::sleep(Duration::from_millis(30));
        }
        std::fs::remove_file(&probe)?;
        lat.sort_by(|a, b| a.partial_cmp(b).unwrap());
        println!("event latency ms: p50 {:.2} p95 {:.2} max {:.2}", lat[15], lat[28], lat[29]);
        return Ok(());
    }

    let quiet = Duration::from_millis(args[3].parse()?);
    println!("READY");
    let (mut total, mut git, mut library, mut work, mut rescans, mut errors) = (0, 0, 0, 0, 0, 0);
    let mut first: Option<Instant> = None;
    let mut last = Instant::now();
    loop {
        match rx.recv_timeout(if first.is_some() { quiet } else { Duration::from_secs(120) }) {
            Ok(Ok(ev)) => {
                first.get_or_insert_with(Instant::now);
                last = Instant::now();
                total += 1;
                if ev.need_rescan() {
                    rescans += 1;
                }
                for p in &ev.paths {
                    match bucket(&root, p) {
                        0 => git += 1,
                        1 => library += 1,
                        _ => work += 1,
                    }
                }
            }
            Ok(Err(e)) => {
                errors += 1;
                eprintln!("watch error: {e}");
            }
            Err(RecvTimeoutError::Timeout) => break,
            Err(e) => return Err(e.into()),
        }
    }
    let span = first.map(|f| last.duration_since(f).as_secs_f64() * 1000.0).unwrap_or(0.0);
    println!(
        "events {total} (paths: .git {git}, Library {library}, worktree {work}) rescans {rescans} errors {errors} span {span:.0} ms"
    );
    Ok(())
}

fn bucket(root: &Path, p: &Path) -> u8 {
    match p.strip_prefix(root).ok().and_then(|r| r.components().next()) {
        Some(c) if c.as_os_str() == ".git" => 0,
        Some(c) if c.as_os_str() == "Library" => 1,
        _ => 2,
    }
}
