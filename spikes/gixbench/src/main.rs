use std::time::Instant;

fn ms(t: Instant) -> f64 {
    t.elapsed().as_secs_f64() * 1000.0
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();
    let path = &args[1];
    let what = args.get(2).map(String::as_str).unwrap_or("all");
    let blame_path = args.get(3).cloned();

    let t = Instant::now();
    let repo = gix::open(path)?;
    println!("open: {:.1} ms", ms(t));

    if what == "all" || what == "status" {
        let t = Instant::now();
        let mut n = 0usize;
        for item in repo.status(gix::progress::Discard)?.into_iter(None)? {
            item?;
            n += 1;
        }
        println!("status: {:.1} ms ({n} items)", ms(t));
    }

    if what == "all" || what == "log" {
        for graph in [true, false] {
            let t = Instant::now();
            let head = repo.head_id()?;
            let mut n = 0usize;
            let mut bytes = 0usize;
            for info in repo
                .rev_walk([head])
                .sorting(gix::revision::walk::Sorting::BreadthFirst)
                .use_commit_graph(graph)
                .all()?
            {
                let info = info?;
                let commit = info.object()?;
                let msg = commit.message_raw_sloppy();
                let author = commit.author()?;
                bytes += msg.len() + author.name.len();
                n += 1;
            }
            println!("log (commit-graph {graph}): {:.1} ms ({n} commits, {bytes} B meta)", ms(t));
        }
        let t = Instant::now();
        let head = repo.head_id()?;
        let mut edges = 0usize;
        let mut n = 0usize;
        for info in repo
            .rev_walk([head])
            .sorting(gix::revision::walk::Sorting::ByCommitTime(Default::default()))
            .use_commit_graph(true)
            .all()?
        {
            edges += info?.parent_ids.len();
            n += 1;
        }
        println!("graph topology only (ids+parents, by time): {:.1} ms ({n} commits, {edges} edges)", ms(t));

        let t = Instant::now();
        let head = repo.head_id()?;
        let first: Vec<_> = repo.rev_walk([head]).all()?.take(500).collect::<Result<_, _>>()?;
        println!("log first 500: {:.1} ms ({} commits)", ms(t), first.len());
    }

    if what == "all" || what == "diff" {
        let t = Instant::now();
        let head = repo.head_commit()?;
        let parent = repo.find_commit(head.parent_ids().next().ok_or("no parent")?)?;
        let changes = repo.diff_tree_to_tree(&parent.tree()?, &head.tree()?, None)?;
        println!("diff HEAD^..HEAD trees: {:.1} ms ({} changes)", ms(t), changes.len());

        let t = Instant::now();
        let base = repo.rev_parse_single("HEAD~200")?.object()?.peel_to_tree()?;
        let changes = repo.diff_tree_to_tree(&base, &head.tree()?, None)?;
        println!("diff HEAD~200..HEAD trees: {:.1} ms ({} changes)", ms(t), changes.len());

        let t = Instant::now();
        let mut opts = gix::diff::Options::default().with_rewrites(None);
        opts.track_path();
        let changes = repo.diff_tree_to_tree(&base, &head.tree()?, opts)?;
        println!("diff HEAD~200..HEAD no renames: {:.1} ms ({} changes)", ms(t), changes.len());
    }

    if let Some(file) = blame_path {
        let t = Instant::now();
        let head = repo.head_id()?;
        let outcome = repo.blame_file(file.as_str().into(), head, Default::default())?;
        println!(
            "blame {file}: {:.1} ms ({} hunks, {} commits traversed)",
            ms(t),
            outcome.entries.len(),
            outcome.statistics.commits_traversed
        );
    }
    Ok(())
}
