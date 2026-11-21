use std::io::Read;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

// usage: gitproc <cwd> <kill: none|child|tree> <cancel_after_ms> -- <git args...>
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();
    let cwd = &args[1];
    let kill = args[2].clone();
    let cancel_after: u64 = args[3].parse()?;
    let git_args = &args[5..];

    let t = Instant::now();
    let mut child = Command::new("git")
        .args(git_args)
        .current_dir(cwd)
        .env("GIT_TERMINAL_PROMPT", "0")
        .env_remove("GIT_CURL_VERBOSE")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()?;
    let pid = child.id();
    let mut err = child.stderr.take().ok_or("no stderr")?;

    let reader = std::thread::spawn(move || {
        let mut buf = [0u8; 4096];
        let mut line = Vec::new();
        let mut updates = 0usize;
        let mut shown = 0usize;
        let mut last_print = Instant::now() - Duration::from_secs(1);
        while let Ok(n) = err.read(&mut buf) {
            if n == 0 {
                break;
            }
            for &b in &buf[..n] {
                if b == b'\r' || b == b'\n' {
                    if !line.is_empty() {
                        let s = String::from_utf8_lossy(&line).to_string();
                        if let Some(p) = parse_progress(&s) {
                            updates += 1;
                            if last_print.elapsed() > Duration::from_millis(700) {
                                println!("  progress {:>6.0} ms  {:<28} {:>3}%  {}", t.elapsed().as_secs_f64() * 1000.0, p.0, p.1, p.2);
                                last_print = Instant::now();
                                shown += 1;
                            }
                        } else {
                            println!("  stderr: {s}");
                        }
                        line.clear();
                    }
                } else {
                    line.push(b);
                }
            }
        }
        (updates, shown)
    });

    let mut status = None;
    while t.elapsed() < Duration::from_millis(cancel_after) || kill == "none" {
        if let Some(s) = child.try_wait()? {
            status = Some(s);
            break;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    if status.is_none() {
        let k = Instant::now();
        match kill.as_str() {
            "child" => child.kill()?,
            "tree" => {
                Command::new("taskkill").args(["/T", "/F", "/PID", &pid.to_string()]).output()?;
            }
            _ => {}
        }
        status = Some(child.wait()?);
        println!("cancel ({kill}) took {:.0} ms", k.elapsed().as_secs_f64() * 1000.0);
    }
    let (updates, _) = reader.join().map_err(|_| "reader panicked")?;
    println!("exit {:?} after {:.0} ms, {updates} progress updates parsed", status.and_then(|s| s.code()), t.elapsed().as_secs_f64() * 1000.0);
    Ok(())
}

// "Receiving objects:  45% (1629/3619), 12.00 MiB | 5.00 MiB/s" -> ("Receiving objects", 45, "(1629/3619), 12.00 MiB | 5.00 MiB/s")
fn parse_progress(s: &str) -> Option<(String, u32, String)> {
    let s = s.trim_start_matches("remote: ");
    let (phase, rest) = s.split_once(':')?;
    let rest = rest.trim_start();
    let (pct, tail) = rest.split_once('%')?;
    let pct: u32 = pct.trim().parse().ok()?;
    Some((phase.trim().to_string(), pct, tail.trim().to_string()))
}
