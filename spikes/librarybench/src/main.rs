use std::sync::Mutex;
use std::time::Instant;

struct Row {
    branch: String,
    ahead_behind: Option<(usize, usize)>,
    dirty: Option<bool>,
    ms: f64,
}

fn count(repo: &gix::Repository, tip: gix::ObjectId, hide: gix::ObjectId) -> Result<usize, Box<dyn std::error::Error>> {
    Ok(repo.rev_walk([tip]).with_hidden([hide]).all()?.count())
}

fn refresh(path: &str, with_dirty: bool) -> Result<Row, Box<dyn std::error::Error>> {
    let t = Instant::now();
    let repo = gix::open(path)?;
    let head = repo.head()?;
    let branch = head
        .referent_name()
        .map(|n| n.shorten().to_string())
        .unwrap_or_else(|| "(detached)".into());
    let mut ahead_behind = None;
    if let (Some(name), Some(id)) = (head.referent_name(), head.id()) {
        if let Some(Ok(upstream)) = repo.branch_remote_tracking_ref_name(name, gix::remote::Direction::Fetch) {
            if let Ok(mut r) = repo.find_reference(upstream.as_ref()) {
                let up = r.peel_to_id()?.detach();
                let local = id.detach();
                ahead_behind = Some((count(&repo, local, up)?, count(&repo, up, local)?));
            }
        }
    }
    let dirty = if with_dirty { Some(repo.is_dirty()?) } else { None };
    Ok(Row { branch, ahead_behind, dirty, ms: t.elapsed().as_secs_f64() * 1000.0 })
}

fn run(paths: &[String], threads: usize, with_dirty: bool) -> (f64, Vec<Row>) {
    let t = Instant::now();
    let next = Mutex::new(0usize);
    let out = Mutex::new(Vec::new());
    std::thread::scope(|s| {
        for _ in 0..threads {
            s.spawn(|| loop {
                let i = {
                    let mut n = next.lock().unwrap();
                    let i = *n;
                    *n += 1;
                    i
                };
                if i >= paths.len() {
                    break;
                }
                match refresh(&paths[i], with_dirty) {
                    Ok(r) => out.lock().unwrap().push((i, r)),
                    Err(e) => eprintln!("{}: {e}", paths[i]),
                }
            });
        }
    });
    let mut rows = out.into_inner().unwrap();
    rows.sort_by_key(|(i, _)| *i);
    (t.elapsed().as_secs_f64() * 1000.0, rows.into_iter().map(|(_, r)| r).collect())
}

fn main() {
    // usage: librarybench <file with one repo path per line> [threads]
    let args: Vec<String> = std::env::args().collect();
    let paths: Vec<String> = std::fs::read_to_string(&args[1]).unwrap().lines().map(str::to_string).collect();
    let threads: usize = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(8);
    println!("{} repos, {threads} threads", paths.len());

    for (label, dirty) in [("refs only (branch, ahead/behind)", false), ("refs + is_dirty", true), ("refs + is_dirty, again", true)] {
        let (ms, rows) = run(&paths, threads, dirty);
        let mut per: Vec<f64> = rows.iter().map(|r| r.ms).collect();
        per.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let p = |q: f64| per[((per.len() as f64 - 1.0) * q) as usize];
        let n_dirty = rows.iter().filter(|r| r.dirty == Some(true)).count();
        let n_ab = rows.iter().filter(|r| r.ahead_behind.is_some()).count();
        println!("{label:36} total {ms:7.0} ms | per repo p50 {:.1} p95 {:.1} max {:.1} ms | dirty {n_dirty}, with upstream {n_ab}", p(0.5), p(0.95), p(1.0));
        if label.ends_with("again") {
            let slow: Vec<_> = rows.iter().zip(&paths).filter(|(r, _)| r.ms > 100.0).map(|(r, p)| format!("{} {:.0} ms ({})", p.rsplit(['/', '\\']).next().unwrap(), r.ms, r.branch)).collect();
            if !slow.is_empty() {
                println!("  slow: {}", slow.join(", "));
            }
        }
    }
    // single-thread reference for the visible-rows case: first 25 repos
    let (ms, _) = run(&paths[..25.min(paths.len())], 1, true);
    println!("{:36} total {ms:7.0} ms", "first 25 repos, 1 thread, + dirty");
}
