use std::io::Write;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};

/// Writes `bytes` to `path` through a temporary file beside it, renamed over
/// `path`: a reader never sees half a file. `durable` syncs it first, so a crash
/// leaves the old file or the new one (state gitgud can't rebuild); caches skip
/// that cost. The temporary name is unique per process and call.
///
/// # Errors
/// The I/O error; the temporary file is removed.
pub(crate) fn write_atomic(path: &Path, bytes: &[u8], durable: bool) -> std::io::Result<()> {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let n = NEXT.fetch_add(1, Ordering::Relaxed);
    let mut name = path.file_name().unwrap_or_default().to_os_string();
    name.push(format!(".{}-{n}.tmp", std::process::id()));
    let tmp = path.with_file_name(name);
    let written = std::fs::File::create(&tmp)
        .and_then(|mut f| {
            f.write_all(bytes)?;
            if durable { f.sync_all() } else { Ok(()) }
        })
        .and_then(|()| std::fs::rename(&tmp, path));
    if written.is_err() {
        let _ = std::fs::remove_file(&tmp);
    }
    written
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn replaces_the_file_and_leaves_no_temporary_behind() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("deep/state.json");
        write_atomic(&file, b"one", true).unwrap();
        write_atomic(&file, b"two", false).unwrap();
        assert_eq!(std::fs::read(&file).unwrap(), b"two");
        let names: Vec<_> = std::fs::read_dir(dir.path().join("deep"))
            .unwrap()
            .map(|e| e.unwrap().file_name())
            .collect();
        assert_eq!(names, ["state.json"]);
    }

    #[test]
    fn writers_at_once_each_land_whole() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("shared.bin");
        std::thread::scope(|s| {
            for i in 0..8u8 {
                let file = &file;
                s.spawn(move || write_atomic(file, &[i; 4096], false).unwrap());
            }
        });
        let bytes = std::fs::read(&file).unwrap();
        assert!(bytes.iter().all(|b| *b == bytes[0]), "one writer's bytes");
    }
}
