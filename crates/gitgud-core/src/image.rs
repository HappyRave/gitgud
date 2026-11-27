use std::io::Cursor;
use std::path::Path;

use gix::bstr::BStr;
use image::{ImageEncoder, ImageFormat, ImageReader};
use image_dds::{ImageFormat as DdsImageFormat, Surface, ddsfile::Dds};
use serde::Deserialize;

use crate::cli::GitCli;
use crate::diff::blob_at;
use crate::lfs::{lfs_fetch_object, lfs_is_hydrated, lfs_object_path, parse_lfs_pointer};
use crate::repo::{Error, Repo, read_err};

/// Which version of a file to read, as the image diff names it.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Version {
    /// In commit `0`.
    Commit(String),
    /// In the first parent of commit `0`.
    Parent(String),
    Head,
    Index,
    /// The file on disk, as is.
    Worktree,
    /// Stage 2 of a conflicted path.
    Yours,
    /// Stage 3 of a conflicted path.
    Theirs,
    /// A property merge's result before it's saved, the rest from the index.
    Draft,
}

/// The MIME type of image files the webview can show, by extension.
#[must_use]
pub fn image_type(path: &str) -> Option<&'static str> {
    let ext = Path::new(path).extension()?.to_str()?.to_ascii_lowercase();
    Some(match ext.as_str() {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "bmp" => "image/bmp",
        "ico" => "image/x-icon",
        "svg" => "image/svg+xml",
        "tga" | "psd" | "dds" | "exr" | "hdr" | "tif" | "tiff" => "image/png",
        _ => return None,
    })
}

/// A prepared texture's place in the memory cache: empty until prepared, then the texture
/// (or `None`: left as is). Locked while it is prepared, so concurrent requests for the
/// same texture (both sides of a compare, a warm-up) wait for one result.
type Slot = std::sync::Arc<std::sync::Mutex<Option<Option<Vec<u8>>>>>;

/// Bytes held, and each texture's slot by content hash, size limit and kind.
type ShrunkCache = (usize, std::collections::HashMap<u64, Slot>);

/// How many times each texture was actually prepared (not found in a cache), for tests.
#[cfg(test)]
static PREPARED: std::sync::LazyLock<std::sync::Mutex<std::collections::HashMap<u64, usize>>> =
    std::sync::LazyLock::new(Default::default);

/// Dropped as a whole past [`SHRUNK_BUDGET`].
static SHRUNK: std::sync::LazyLock<std::sync::Mutex<ShrunkCache>> =
    std::sync::LazyLock::new(Default::default);
const SHRUNK_BUDGET: usize = 256 * 1024 * 1024;

/// An encoded image shrunk so its longer side is at most `max` px, as fast-compressed
/// PNG, for 3D previews: big textures cost gigabytes and seconds of GPU upload there.
/// `None` when it is already small enough or can't be decoded, so the original is used.
#[must_use]
pub fn shrink_image(bytes: &[u8], max: u32) -> Option<Vec<u8>> {
    prepare(bytes, max, Prepared::Png, None, &mut || Ok(bytes)).unwrap_or(None)
}

/// A texture for 3D previews.
#[derive(Debug, PartialEq, Eq)]
pub enum PreviewTexture {
    /// GPU-compressed, as DDS.
    Dds(Vec<u8>),
    /// Shrunk, as PNG.
    Png(Vec<u8>),
    /// The image as it is: small enough, or not something to compress or shrink.
    Original(Vec<u8>),
}

/// What a 3D preview texture is turned into.
#[derive(Clone, Copy)]
enum Prepared {
    /// Shrunk, as PNG ([`shrink_image`]).
    Png,
    /// GPU-compressed, as DDS ([`crate::gpu_texture::bc_dds`]).
    Bc,
}

fn texture_key(source: &[u8], max: u32, kind: Prepared) -> u64 {
    xxhash_rust::xxh64::xxh64(source, u64::from(max) | ((kind as u64) << 32))
}

/// The memory cache's slot for `key`, made when there is none.
fn slot(key: u64) -> Slot {
    SHRUNK
        .lock()
        .expect("shrunk lock poisoned")
        .1
        .entry(key)
        .or_default()
        .clone()
}

/// Counts a just-filled slot's texture against the memory budget.
fn account(out: Option<&Vec<u8>>) {
    let mut cache = SHRUNK.lock().expect("shrunk lock poisoned");
    cache.0 += out.map_or(0, Vec::len);
    if cache.0 > SHRUNK_BUDGET {
        *cache = Default::default();
    }
}

/// The texture `source` (what identifies the image) gives as `kind`: from the memory
/// cache, else `disk`, else prepared from `bytes()`, which only runs then and whose
/// error is returned without keeping anything.
fn prepare<'b>(
    source: &[u8],
    max: u32,
    kind: Prepared,
    disk: Option<&crate::disk_cache::DiskCache>,
    bytes: &mut dyn FnMut() -> Result<&'b [u8], Error>,
) -> Result<Option<Vec<u8>>, Error> {
    let key = texture_key(source, max, kind);
    let slot = slot(key);
    let mut held = slot.lock().expect("texture slot poisoned");
    if let Some(out) = held.as_ref() {
        return Ok(out.clone());
    }
    let out = match disk.and_then(|d| d.texture(key)) {
        Some(hit) => hit,
        None => {
            let bytes = bytes()?;
            #[cfg(test)]
            {
                *PREPARED.lock().unwrap().entry(key).or_default() += 1;
            }
            let out = match kind {
                Prepared::Png => shrink_uncached(bytes, max),
                Prepared::Bc => crate::gpu_texture::bc_dds(bytes, max),
            };
            if let Some(d) = disk {
                d.put_texture(key, out.as_deref());
            }
            out
        }
    };
    account(out.as_ref());
    *held = Some(out.clone());
    Ok(out)
}

/// Drops the shrunk textures kept in memory.
pub(crate) fn forget_shrunk() {
    *SHRUNK.lock().expect("shrunk lock poisoned") = Default::default();
}

fn shrink_uncached(bytes: &[u8], max: u32) -> Option<Vec<u8>> {
    image::load_from_memory(bytes).ok().and_then(|img| {
        let (w, h) = (img.width(), img.height());
        if w.max(h) <= max {
            return None;
        }
        let scale = f64::from(max) / f64::from(w.max(h));
        let (sw, sh) = (
            ((f64::from(w) * scale).round() as u32).max(1),
            ((f64::from(h) * scale).round() as u32).max(1),
        );
        let small = image::imageops::thumbnail(&img, sw, sh);
        let mut png = Vec::new();
        image::codecs::png::PngEncoder::new_with_quality(
            &mut png,
            image::codecs::png::CompressionType::Fast,
            image::codecs::png::FilterType::Adaptive,
        )
        .write_image(small.as_raw(), sw, sh, image::ExtendedColorType::Rgba8)
        .ok()
        .map(|()| png)
    })
}

impl Repo {
    /// [`shrink_image`], kept in the repo's disk cache too, so 3D previews skip the
    /// work after a restart.
    #[must_use]
    pub fn shrink_texture(&self, bytes: &[u8], max: u32) -> Option<Vec<u8>> {
        let disk = crate::disk_cache::DiskCache::new(self);
        prepare(bytes, max, Prepared::Png, Some(&disk), &mut || Ok(bytes)).unwrap_or(None)
    }

    /// [`crate::gpu_texture::bc_dds`], kept in memory and in the repo's disk cache.
    #[must_use]
    pub fn compressed_texture(&self, bytes: &[u8], max: u32) -> Option<Vec<u8>> {
        let disk = crate::disk_cache::DiskCache::new(self);
        prepare(bytes, max, Prepared::Bc, Some(&disk), &mut || Ok(bytes)).unwrap_or(None)
    }

    /// The image `decode` gives, for 3D previews: GPU-compressed when `bc` and it can be,
    /// else shrunk to `max`, else as it is. Kept in memory and on disk under `stored`,
    /// the file as git stores it (its content, or its LFS pointer), so a kept texture
    /// is found without reading an LFS object or decoding a TGA / PSD.
    ///
    /// # Errors
    /// What `decode` returns, when it has to run.
    pub fn preview_texture(
        &self,
        stored: &[u8],
        max: u32,
        bc: bool,
        decode: impl FnOnce() -> Result<Vec<u8>, Error>,
    ) -> Result<PreviewTexture, Error> {
        let disk = crate::disk_cache::DiskCache::new(self);
        let kinds: &[Prepared] = if bc {
            &[Prepared::Bc, Prepared::Png]
        } else {
            &[Prepared::Png]
        };
        let wrap = |kind: Prepared, out: Vec<u8>| match kind {
            Prepared::Bc => PreviewTexture::Dds(out),
            Prepared::Png => PreviewTexture::Png(out),
        };
        // Decoded at most once, and only when a kind isn't kept yet.
        let decoded = std::cell::OnceCell::new();
        let mut decode = Some(decode);
        let mut bytes = || -> Result<&[u8], Error> {
            if decoded.get().is_none() {
                let run = decode.take().expect("decoded once");
                let _ = decoded.set(run()?);
            }
            Ok(decoded.get().expect("just decoded").as_slice())
        };
        for &kind in kinds {
            if let Some(out) = prepare(stored, max, kind, Some(&disk), &mut bytes)? {
                return Ok(wrap(kind, out));
            }
        }
        bytes()?;
        Ok(PreviewTexture::Original(
            decoded.into_inner().expect("just decoded"),
        ))
    }

    /// The bytes of image `path` at `version`; an LFS pointer is swapped for its
    /// local object.
    ///
    /// # Errors
    /// [`Error::Refused`] when `path` isn't an image, doesn't exist in that
    /// version, or its LFS object isn't downloaded; [`Error::Read`] otherwise.
    pub fn image_bytes(&self, path: &str, version: &Version) -> Result<Vec<u8>, Error> {
        let stored = self.stored_image(path, version)?;
        let resolved = self.lfs_resolve_with_fetch(None, path, stored, 0)?;
        decode_image(path, resolved)
    }

    /// Image `path` at `version` as git stores it: its content, or its LFS pointer.
    ///
    /// # Errors
    /// As [`Repo::image_bytes`], short of reading the LFS object.
    pub fn stored_image(&self, path: &str, version: &Version) -> Result<Vec<u8>, Error> {
        if image_type(path).is_none() {
            return Err(Error::Refused(format!("{path} is not an image")));
        }
        if *version == Version::Worktree
            && let Some(pointer) = self.unchanged_lfs_pointer(path)
        {
            return Ok(pointer);
        }
        self.file_at(path, version)
    }

    /// The index's LFS pointer for `path` when the work-tree file is unchanged since it
    /// was staged (git's stat check, racy entries excluded), so the working copy's
    /// texture is kept under the same key as HEAD's.
    fn unchanged_lfs_pointer(&self, path: &str) -> Option<Vec<u8>> {
        let repo = self.gix();
        let index = repo.index_or_empty().ok()?;
        let entry = index.entry_by_path(BStr::new(path))?;
        let options = gix::index::entry::stat::Options::default();
        if entry.stat.is_racy(index.timestamp(), options) {
            return None;
        }
        let meta = gix::index::fs::Metadata::from_path_no_follow(&self.root().join(path)).ok()?;
        let on_disk = gix::index::entry::Stat::from_fs(&meta).ok()?;
        if !entry.stat.matches(&on_disk, options) {
            return None;
        }
        let blob = repo.find_blob(entry.id).ok()?;
        let (_, size) = parse_lfs_pointer(&blob.data)?;
        (size == meta.len()).then(|| blob.data.clone())
    }

    /// The raw bytes of any file `path` at `version` (LFS pointers are not resolved).
    /// A gitignored file has no versions, so every version reads its disk copy.
    ///
    /// # Errors
    /// [`Error::Refused`] when `path` escapes the repo, is inside `.git`, or isn't in
    /// that version; [`Error::Read`] otherwise.
    pub fn file_at(&self, path: &str, version: &Version) -> Result<Vec<u8>, Error> {
        // Windows opens `.git.`, `.git ` and the short name `GIT~1` as `.git`.
        let is_git_dir = |name: &std::ffi::OsStr| {
            let name = name.to_string_lossy();
            let trimmed = name.trim_end_matches(['.', ' ']);
            trimmed.eq_ignore_ascii_case(".git") || trimmed.to_ascii_uppercase().starts_with("GIT~")
        };
        let inside = Path::new(path).components().all(|c| match c {
            std::path::Component::Normal(name) => !is_git_dir(name),
            _ => false,
        });
        if !inside {
            return Err(Error::Refused(format!(
                "{path} is not a path inside the repo"
            )));
        }
        match self.versioned_file(path, version) {
            Err(Error::Refused(m)) if self.ignored_on_disk(path) => {
                std::fs::read(self.root().join(path)).map_err(|_| Error::Refused(m))
            }
            found => found,
        }
    }

    /// Whether `path` is a gitignored file present on disk (an asset pack kept out of
    /// git): such a file is read from disk in every version. Kept until the next refresh.
    pub fn ignored_on_disk(&self, path: &str) -> bool {
        let ancestors = || {
            path.match_indices('/')
                .map(|(i, _)| format!("{}/", &path[..i]))
        };
        {
            let known = self.ignored.lock().expect("ignored lock poisoned");
            if let Some(&k) = known.get(path) {
                return k;
            }
            if ancestors().any(|dir| known.get(&dir) == Some(&true)) {
                return self.root().join(path).is_file();
            }
        }
        let mut ignored_dir = None;
        let ignored = self.root().join(path).is_file() && {
            let gix = self.gix();
            gix.index_or_empty().ok().is_some_and(|index| {
                if index.entry_by_path(BStr::new(path)).is_some() {
                    return false;
                }
                let Ok(mut ex) = gix.excludes(&index, None, Default::default()) else {
                    return false;
                };
                let mut excluded = |p: &str, mode| {
                    ex.at_path(Path::new(p), Some(mode))
                        .is_ok_and(|e| e.is_excluded())
                };
                if !excluded(path, gix::index::entry::Mode::FILE) {
                    return false;
                }
                // Remember the outermost ignored folder, so its other files skip this check.
                ignored_dir = ancestors()
                    .find(|dir| excluded(dir.trim_end_matches('/'), gix::index::entry::Mode::DIR));
                true
            })
        };
        let mut known = self.ignored.lock().expect("ignored lock poisoned");
        known.insert(path.to_owned(), ignored);
        if let Some(dir) = ignored_dir {
            known.insert(dir, true);
        }
        ignored
    }

    fn versioned_file(&self, path: &str, version: &Version) -> Result<Vec<u8>, Error> {
        let missing = || Error::Refused(format!("{path} is not in that version"));
        match version {
            Version::Worktree => match std::fs::read(self.root().join(path)) {
                Ok(d) => Ok(d),
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => Err(missing()),
                Err(e) => Err(read_err(e)),
            },
            Version::Commit(id) | Version::Parent(id) => {
                let repo = self.gix();
                let oid = gix::ObjectId::from_hex(id.as_bytes())
                    .map_err(|_| Error::Read(format!("{id:?} is not a commit id")))?;
                let mut commit = repo.find_commit(oid).map_err(read_err)?;
                if matches!(version, Version::Parent(_)) {
                    let Some(parent) = commit.parent_ids().next() else {
                        return Err(missing());
                    };
                    commit = parent.object().map_err(read_err)?.into_commit();
                }
                blob_at(&repo, &commit, path)?.ok_or_else(missing)
            }
            Version::Head => {
                let repo = self.gix();
                match repo.head_commit() {
                    Ok(commit) => blob_at(&repo, &commit, path)?.ok_or_else(missing),
                    Err(_) if repo.head().map_err(read_err)?.is_unborn() => Err(missing()),
                    Err(e) => Err(read_err(e)),
                }
            }
            Version::Index => {
                let repo = self.gix();
                let index = repo.index_or_empty().map_err(read_err)?;
                match index.entry_by_path(BStr::new(path)) {
                    Some(e) => Ok(repo.find_blob(e.id).map_err(read_err)?.data.clone()),
                    None => Err(missing()),
                }
            }
            Version::Yours | Version::Theirs => {
                let [_, yours, theirs] = self.stages(path)?;
                let picked = if matches!(version, Version::Yours) {
                    yours
                } else {
                    theirs
                };
                picked.ok_or_else(missing)
            }
            Version::Draft => match self.draft(path) {
                Some(bytes) => Ok((*bytes).clone()),
                None => self.versioned_file(path, &Version::Index),
            },
        }
    }

    pub(crate) fn lfs_resolve_with_fetch(
        &self,
        git: Option<&GitCli>,
        path: &str,
        data: Vec<u8>,
        auto_fetch_max_bytes: u64,
    ) -> Result<Vec<u8>, Error> {
        if !crate::lfs::has_lfs_header(&data) {
            return Ok(data);
        }
        let (oid, size) = parse_lfs_pointer(&data)
            .ok_or_else(|| Error::Read(format!("{path} has a malformed LFS pointer")))?;
        if !lfs_is_hydrated(self, &oid) {
            if auto_fetch_max_bytes > 0 && size > auto_fetch_max_bytes {
                return Err(Error::Refused(format!(
                    "{path}: LFS object ({}) isn't downloaded — click Fetch to download",
                    format_bytes(size)
                )));
            }
            if auto_fetch_max_bytes > 0
                && let Some(git) = git
                && self.primary_remote_url().is_some()
            {
                lfs_fetch_object(git, self, &oid, size, Some(path))?;
            }
        }
        let object = lfs_object_path(self, &oid);
        match std::fs::read(&object) {
            Ok(d) => Ok(d),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Err(Error::Refused(format!(
                "{path}: this LFS version isn't downloaded"
            ))),
            Err(e) => Err(read_err(e)),
        }
    }
}

/// Image `path`'s bytes (its LFS object in place) as the webview can show them: TGA,
/// PSD, DDS, EXR, HDR and TIFF decoded to PNG, the rest as they are.
///
/// # Errors
/// [`Error::Read`] when a format decoded here doesn't decode.
pub fn decode_image(path: &str, bytes: Vec<u8>) -> Result<Vec<u8>, Error> {
    let ext = Path::new(path)
        .extension()
        .and_then(|e| e.to_str())
        .map(str::to_ascii_lowercase);
    let (decoded, kind) = match ext.as_deref() {
        Some("tga") => (decode_tga_to_png(&bytes), "TGA"),
        Some("psd") => (decode_psd_to_png(&bytes), "PSD"),
        Some("dds") => (decode_dds_to_png(&bytes), "DDS"),
        Some("exr") => (
            decode_hdr_format_to_png(&bytes, ImageFormat::OpenExr),
            "EXR",
        ),
        Some("hdr") => (decode_hdr_format_to_png(&bytes, ImageFormat::Hdr), "HDR"),
        Some("tif" | "tiff") => (decode_tiff_to_png(&bytes), "TIFF"),
        _ => return Ok(bytes),
    };
    decoded.map_err(|e| Error::Read(format!("{path}: invalid {kind} ({e})")))
}

fn format_bytes(bytes: u64) -> String {
    if bytes < 1024 {
        return format!("{bytes} B");
    }
    let units = ["KB", "MB", "GB"];
    let mut n = bytes as f64 / 1024.0;
    let mut u = 0usize;
    while n >= 1024.0 && u < units.len() - 1 {
        n /= 1024.0;
        u += 1;
    }
    if n < 10.0 {
        format!("{n:.1} {}", units[u])
    } else {
        format!("{n:.0} {}", units[u])
    }
}

const MAX_PIXELS: usize = 33_554_432;

fn decode_tga_to_png(data: &[u8]) -> Result<Vec<u8>, String> {
    if data.len() < 18 {
        return Err("truncated header".into());
    }
    let id_len = data[0] as usize;
    let color_map_type = data[1];
    let image_type = data[2];
    if color_map_type != 0 {
        return Err("color-mapped TGA is unsupported".into());
    }
    let cmap_len = u16::from_le_bytes([data[5], data[6]]) as usize;
    let cmap_entry_bytes = (data[7] as usize).div_ceil(8);
    let width = u16::from_le_bytes([data[12], data[13]]) as u32;
    let height = u16::from_le_bytes([data[14], data[15]]) as u32;
    let bpp = data[16];
    let descriptor = data[17];
    if width == 0 || height == 0 || (width as usize) * (height as usize) > MAX_PIXELS {
        return Err("invalid dimensions".into());
    }
    let bytes_per_pixel = match bpp {
        8 => 1usize,
        24 => 3,
        32 => 4,
        _ => return Err(format!("unsupported bit depth {bpp}")),
    };
    let pixel_count = (width as usize) * (height as usize);
    // RLE packs at most 128 pixels per packet: a short file can't hold a big image,
    // so it is refused before the pixel buffer is allocated.
    if pixel_count > data.len().saturating_mul(128) {
        return Err("truncated: too little data for its dimensions".into());
    }
    let mut offset = 18 + id_len + cmap_len * cmap_entry_bytes;
    if offset > data.len() {
        return Err("truncated pixel offset".into());
    }

    let rle = matches!(image_type, 10 | 11);
    let gray = matches!(image_type, 3 | 11);
    if !matches!(image_type, 2 | 3 | 10 | 11) || (gray && bytes_per_pixel != 1) {
        return Err(format!("unsupported TGA image type {image_type}"));
    }

    let mut raw = Vec::with_capacity(pixel_count * bytes_per_pixel);
    if !rle {
        let need = pixel_count * bytes_per_pixel;
        if offset + need > data.len() {
            return Err("truncated uncompressed pixel data".into());
        }
        raw.extend_from_slice(&data[offset..offset + need]);
    } else {
        while raw.len() < pixel_count * bytes_per_pixel {
            if offset >= data.len() {
                return Err("truncated RLE packet".into());
            }
            let packet = data[offset];
            offset += 1;
            let count = ((packet & 0x7f) as usize) + 1;
            if packet & 0x80 != 0 {
                if offset + bytes_per_pixel > data.len() {
                    return Err("truncated RLE run pixel".into());
                }
                let px = &data[offset..offset + bytes_per_pixel];
                offset += bytes_per_pixel;
                for _ in 0..count {
                    if raw.len() >= pixel_count * bytes_per_pixel {
                        break;
                    }
                    raw.extend_from_slice(px);
                }
            } else {
                let need = count * bytes_per_pixel;
                if offset + need > data.len() {
                    return Err("truncated RLE raw packet".into());
                }
                let take = need.min(pixel_count * bytes_per_pixel - raw.len());
                raw.extend_from_slice(&data[offset..offset + take]);
                offset += need;
            }
        }
    }

    let top_down = descriptor & 0x20 != 0;
    let right_to_left = descriptor & 0x10 != 0;
    let mut rgba = vec![0u8; pixel_count * 4];
    for y in 0..height as usize {
        let dst_y = if top_down { y } else { height as usize - 1 - y };
        for x in 0..width as usize {
            let dst_x = if right_to_left {
                width as usize - 1 - x
            } else {
                x
            };
            let src_i = (y * width as usize + x) * bytes_per_pixel;
            let dst_i = (dst_y * width as usize + dst_x) * 4;
            match bytes_per_pixel {
                1 => {
                    let g = raw[src_i];
                    rgba[dst_i..dst_i + 4].copy_from_slice(&[g, g, g, 255]);
                }
                3 => {
                    rgba[dst_i..dst_i + 4].copy_from_slice(&[
                        raw[src_i + 2],
                        raw[src_i + 1],
                        raw[src_i],
                        255,
                    ]);
                }
                4 => {
                    rgba[dst_i..dst_i + 4].copy_from_slice(&[
                        raw[src_i + 2],
                        raw[src_i + 1],
                        raw[src_i],
                        raw[src_i + 3],
                    ]);
                }
                _ => {}
            }
        }
    }

    encode_rgba_png(width, height, &rgba)
}

fn decode_psd_to_png(data: &[u8]) -> Result<Vec<u8>, String> {
    if data.len() < 26 || &data[0..4] != b"8BPS" {
        return Err("missing 8BPS header".into());
    }
    let version = u16::from_be_bytes([data[4], data[5]]);
    if version != 1 {
        return Err(format!("unsupported PSD version {version}"));
    }
    let channels = u16::from_be_bytes([data[12], data[13]]) as usize;
    let height = u32::from_be_bytes([data[14], data[15], data[16], data[17]]);
    let width = u32::from_be_bytes([data[18], data[19], data[20], data[21]]);
    let depth = u16::from_be_bytes([data[22], data[23]]);
    let color_mode = u16::from_be_bytes([data[24], data[25]]);
    if !matches!(depth, 8 | 16) || !matches!(color_mode, 1 | 3) || channels == 0 {
        return Err("only 8/16-bit Grayscale/RGB flat PSDs are supported".into());
    }
    if width == 0 || height == 0 || (width as usize) * (height as usize) > MAX_PIXELS {
        return Err("invalid PSD dimensions".into());
    }

    let mut pos = 26usize;
    for _ in 0..3 {
        if pos + 4 > data.len() {
            return Err("truncated section length".into());
        }
        let len =
            u32::from_be_bytes([data[pos], data[pos + 1], data[pos + 2], data[pos + 3]]) as usize;
        pos = pos
            .checked_add(4 + len)
            .filter(|&p| p <= data.len())
            .ok_or_else(|| "truncated section body".to_owned())?;
    }

    if pos + 2 > data.len() {
        return Err("missing compression method".into());
    }
    let compression = u16::from_be_bytes([data[pos], data[pos + 1]]);
    pos += 2;
    let pixels = (width as usize) * (height as usize);
    // PackBits expands at most 128 times: refuse a short file before allocating planes.
    if pixels > (data.len() - pos).saturating_mul(128) {
        return Err("truncated: too little data for its dimensions".into());
    }
    let sample_bytes = usize::from(depth / 8);
    let row_bytes = (width as usize) * sample_bytes;
    let plane_bytes = pixels * sample_bytes;
    let used_channels = channels.min(4);
    let mut planes = vec![vec![255u8; pixels]; 4];

    match compression {
        0 => {
            let need = plane_bytes
                .checked_mul(channels)
                .ok_or_else(|| "PSD channel size overflow".to_owned())?;
            if pos + need > data.len() {
                return Err("truncated raw PSD data".into());
            }
            for (c, plane) in planes.iter_mut().enumerate().take(used_channels) {
                let start = pos + c * plane_bytes;
                if sample_bytes == 1 {
                    plane.copy_from_slice(&data[start..start + pixels]);
                } else {
                    for (i, dst) in plane.iter_mut().enumerate() {
                        *dst = data[start + i * 2];
                    }
                }
            }
        }
        1 => {
            let row_count = channels * (height as usize);
            if pos + row_count * 2 > data.len() {
                return Err("truncated PackBits line table".into());
            }
            let mut line_lens = Vec::with_capacity(row_count);
            for i in 0..row_count {
                let b = pos + i * 2;
                line_lens.push(u16::from_be_bytes([data[b], data[b + 1]]) as usize);
            }
            pos += row_count * 2;
            let mut row_buf = if sample_bytes == 2 {
                vec![0u8; row_bytes]
            } else {
                Vec::new()
            };
            for c in 0..channels {
                for y in 0..height as usize {
                    let len = line_lens[c * height as usize + y];
                    if pos + len > data.len() {
                        return Err("truncated PackBits scanline".into());
                    }
                    let scan = &data[pos..pos + len];
                    pos += len;
                    if c < used_channels {
                        let dst = &mut planes[c][y * width as usize..(y + 1) * width as usize];
                        if sample_bytes == 1 {
                            unpack_bits(scan, dst)?;
                        } else {
                            row_buf.fill(0);
                            unpack_bits(scan, &mut row_buf)?;
                            for (x, px) in dst.iter_mut().enumerate() {
                                *px = row_buf[x * 2];
                            }
                        }
                    }
                }
            }
        }
        other => return Err(format!("unsupported PSD compression {other}")),
    }

    let mut rgba = vec![255u8; pixels * 4];
    let (px_chunks, _) = rgba.as_chunks_mut::<4>();
    for (i, dst) in px_chunks.iter_mut().enumerate() {
        if color_mode == 1 {
            let g = planes[0][i];
            dst[0] = g;
            dst[1] = g;
            dst[2] = g;
            dst[3] = if channels >= 2 { planes[1][i] } else { 255 };
        } else {
            dst[0] = planes[0][i];
            dst[1] = if used_channels >= 2 { planes[1][i] } else { 0 };
            dst[2] = if used_channels >= 3 { planes[2][i] } else { 0 };
            dst[3] = if used_channels >= 4 {
                planes[3][i]
            } else {
                255
            };
        }
    }
    encode_rgba_png(width, height, &rgba)
}

fn decode_dds_to_png(data: &[u8]) -> Result<Vec<u8>, String> {
    let mut cursor = Cursor::new(data);
    let dds = Dds::read(&mut cursor).map_err(|e| e.to_string())?;
    let surface = Surface::from_dds(&dds).map_err(|e| e.to_string())?;
    let pixels = (surface.width as usize).checked_mul(surface.height as usize);
    if surface.width == 0 || surface.height == 0 || pixels.is_none_or(|p| p > MAX_PIXELS) {
        return Err("invalid DDS dimensions".into());
    }
    if is_float_dds_format(surface.image_format) {
        let img = surface
            .decode_layers_mipmaps_rgbaf32(0..1, 0..1)
            .map_err(|e| e.to_string())?
            .into_image()
            .map_err(|e| e.to_string())?;
        let rgba = tonemap_rgba32f_to_rgba8(img.as_raw());
        encode_rgba_png(img.width(), img.height(), &rgba)
    } else {
        let img = surface
            .decode_layers_mipmaps_rgba8(0..1, 0..1)
            .map_err(|e| e.to_string())?
            .into_image()
            .map_err(|e| e.to_string())?;
        encode_rgba_png(img.width(), img.height(), img.as_raw())
    }
}

fn is_float_dds_format(fmt: DdsImageFormat) -> bool {
    matches!(
        fmt,
        DdsImageFormat::R16Float
            | DdsImageFormat::Rg16Float
            | DdsImageFormat::Rgba16Float
            | DdsImageFormat::R32Float
            | DdsImageFormat::Rg32Float
            | DdsImageFormat::Rgb32Float
            | DdsImageFormat::Rgba32Float
            | DdsImageFormat::BC6hRgbUfloat
            | DdsImageFormat::BC6hRgbSfloat
    )
}

fn check_reader_dimensions(data: &[u8], format: ImageFormat) -> Result<(), String> {
    let reader = ImageReader::with_format(Cursor::new(data), format);
    let (w, h) = reader.into_dimensions().map_err(|e| e.to_string())?;
    let pixels = (w as usize).checked_mul(h as usize);
    if w == 0 || h == 0 || pixels.is_none_or(|p| p > MAX_PIXELS) {
        return Err("invalid dimensions".into());
    }
    Ok(())
}

fn decode_hdr_format_to_png(data: &[u8], format: ImageFormat) -> Result<Vec<u8>, String> {
    check_reader_dimensions(data, format)?;
    let dyn_img = image::load_from_memory_with_format(data, format).map_err(|e| e.to_string())?;
    let rgba32f = dyn_img.into_rgba32f();
    let rgba = tonemap_rgba32f_to_rgba8(rgba32f.as_raw());
    encode_rgba_png(rgba32f.width(), rgba32f.height(), &rgba)
}

fn decode_tiff_to_png(data: &[u8]) -> Result<Vec<u8>, String> {
    check_reader_dimensions(data, ImageFormat::Tiff)?;
    let dyn_img =
        image::load_from_memory_with_format(data, ImageFormat::Tiff).map_err(|e| e.to_string())?;
    let rgba = dyn_img.into_rgba8();
    encode_rgba_png(rgba.width(), rgba.height(), rgba.as_raw())
}

fn tonemap_rgba32f_to_rgba8(raw: &[f32]) -> Vec<u8> {
    let mut out = Vec::with_capacity(raw.len());
    let (pixels, _) = raw.as_chunks::<4>();
    for px in pixels {
        for &c in &px[..3] {
            let v = if c.is_finite() && c > 0.0 { c } else { 0.0 };
            let mapped = v / (1.0 + v);
            let srgb = mapped.powf(1.0 / 2.2).clamp(0.0, 1.0);
            out.push((srgb * 255.0 + 0.5) as u8);
        }
        let a = if px[3].is_finite() {
            px[3].clamp(0.0, 1.0)
        } else {
            1.0
        };
        out.push((a * 255.0 + 0.5) as u8);
    }
    out
}

fn unpack_bits(src: &[u8], dst: &mut [u8]) -> Result<(), String> {
    let mut si = 0usize;
    let mut di = 0usize;
    while di < dst.len() && si < src.len() {
        let header = src[si] as i8;
        si += 1;
        if header >= 0 {
            let count = (header as usize) + 1;
            if si + count > src.len() {
                return Err("truncated PackBits literal".into());
            }
            let take = count.min(dst.len() - di);
            dst[di..di + take].copy_from_slice(&src[si..si + take]);
            si += count;
            di += take;
        } else if header != -128 {
            let count = ((-i16::from(header)) as usize) + 1;
            if si >= src.len() {
                return Err("truncated PackBits repeat".into());
            }
            let val = src[si];
            si += 1;
            let take = count.min(dst.len() - di);
            dst[di..di + take].fill(val);
            di += take;
        }
    }
    Ok(())
}

/// `rgba` as fast-compressed PNG.
fn encode_rgba_png(width: u32, height: u32, rgba: &[u8]) -> Result<Vec<u8>, String> {
    let mut png = Vec::new();
    image::codecs::png::PngEncoder::new_with_quality(
        &mut png,
        image::codecs::png::CompressionType::Fast,
        image::codecs::png::FilterType::Adaptive,
    )
    .write_image(rgba, width, height, image::ExtendedColorType::Rgba8)
    .map_err(|e| e.to_string())?;
    Ok(png)
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::GenericImageView;

    /// Tests that clear or count the shared texture cache run one at a time: a
    /// clear in one would make another prepare a texture twice.
    fn shared_cache() -> std::sync::MutexGuard<'static, ()> {
        static ONE: std::sync::Mutex<()> = std::sync::Mutex::new(());
        ONE.lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    #[test]
    fn shrinks_big_textures_for_3d_and_leaves_small_ones() {
        let _one = shared_cache();
        let encode = |w: u32, h: u32| {
            let img =
                image::RgbaImage::from_fn(w, h, |x, _| image::Rgba([(x % 256) as u8, 0, 0, 255]));
            let mut out = Cursor::new(Vec::new());
            img.write_to(&mut out, ImageFormat::Png).unwrap();
            out.into_inner()
        };
        let big = encode(4096, 2048);
        let small = shrink_image(&big, 1024).unwrap();
        let back = image::load_from_memory(&small).unwrap();
        assert_eq!((back.width(), back.height()), (1024, 512));
        assert_eq!(shrink_image(&big, 1024), Some(small), "cached");
        assert_eq!(shrink_image(&encode(800, 600), 1024), None, "already small");
        assert_eq!(shrink_image(b"not an image", 1024), None);
        let mut jpg = Cursor::new(Vec::new());
        image::RgbImage::new(2000, 10)
            .write_to(&mut jpg, ImageFormat::Jpeg)
            .unwrap();
        let shrunk = shrink_image(&jpg.into_inner(), 1024).unwrap();
        assert!(
            shrunk.starts_with(b"\x89PNG"),
            "JPEG decodes and comes back as PNG"
        );
    }

    #[test]
    fn keeps_shrunk_textures_on_disk() {
        let _one = shared_cache();
        let repo = crate::testing::TestRepo::init();
        let r = crate::repo::open(repo.path()).unwrap();
        let mut big = Cursor::new(Vec::new());
        image::RgbaImage::from_fn(2048, 64, |x, y| image::Rgba([x as u8, y as u8, 7, 255]))
            .write_to(&mut big, ImageFormat::Png)
            .unwrap();
        let big = big.into_inner();
        let small = r.shrink_texture(&big, 512).unwrap();
        assert!(r.shrink_texture(b"tiny", 512).is_none());
        let disk = crate::disk_cache::DiskCache::new(&r);
        assert_eq!(
            disk.stats().textures,
            2,
            "the shrunk one and the one left as is"
        );
        forget_shrunk();
        assert_eq!(
            r.shrink_texture(&big, 512),
            Some(small),
            "read back from disk"
        );
    }

    #[test]
    fn a_kept_preview_texture_is_found_by_the_stored_file_without_decoding() {
        let _one = shared_cache();
        let repo = crate::testing::TestRepo::init();
        let r = crate::repo::open(repo.path()).unwrap();
        let png = |w: u32, seed: u8| {
            let mut out = Cursor::new(Vec::new());
            image::RgbaImage::from_fn(w, 32, |x, y| image::Rgba([x as u8, y as u8, seed, 255]))
                .write_to(&mut out, ImageFormat::Png)
                .unwrap();
            out.into_inner()
        };
        let (big, small) = (png(1500, 3), png(64, 4));
        let stored = b"version https://git-lfs.github.com/spec/v1\noid sha256:aa\nsize 9\n";
        let never = || -> Result<Vec<u8>, Error> { panic!("decoded although it was kept") };

        let shrunk = r
            .preview_texture(stored, 512, false, || Ok(big.clone()))
            .unwrap();
        assert!(matches!(&shrunk, PreviewTexture::Png(p) if p.starts_with(b"\x89PNG")));
        assert_eq!(
            r.preview_texture(stored, 512, false, never).unwrap(),
            shrunk
        );
        forget_shrunk();
        assert_eq!(
            r.preview_texture(stored, 512, false, never).unwrap(),
            shrunk,
            "from disk"
        );

        let dds = r
            .preview_texture(b"other pointer", 512, true, || Ok(big.clone()))
            .unwrap();
        assert!(matches!(dds, PreviewTexture::Dds(_)));
        forget_shrunk();
        assert_eq!(
            r.preview_texture(b"other pointer", 512, true, never)
                .unwrap(),
            dds
        );

        assert_eq!(
            r.preview_texture(b"small one", 512, false, || Ok(small.clone()))
                .unwrap(),
            PreviewTexture::Original(small.clone()),
            "small enough: the image as it is"
        );
    }

    #[test]
    fn prepares_a_texture_once_for_concurrent_requests() {
        let _one = shared_cache();
        let repo = crate::testing::TestRepo::init();
        let r = crate::repo::open(repo.path()).unwrap();
        let mut png = Cursor::new(Vec::new());
        image::RgbaImage::from_fn(1500, 40, |x, y| image::Rgba([x as u8, y as u8, 99, 255]))
            .write_to(&mut png, ImageFormat::Png)
            .unwrap();
        let png = png.into_inner();
        let outs: Vec<_> = std::thread::scope(|s| {
            let handles: Vec<_> = (0..8)
                .map(|_| s.spawn(|| r.compressed_texture(&png, 512)))
                .collect();
            handles.into_iter().map(|h| h.join().unwrap()).collect()
        });
        assert!(outs[0].is_some());
        assert!(outs.iter().all(|o| *o == outs[0]));
        let key = xxhash_rust::xxh64::xxh64(&png, 512 | ((Prepared::Bc as u64) << 32));
        let prepared = PREPARED.lock().unwrap().get(&key).copied();
        assert_eq!(prepared, Some(1));
    }

    #[test]
    fn image_types_by_extension() {
        assert_eq!(image_type("a/b.PNG"), Some("image/png"));
        assert_eq!(image_type("t.jpeg"), Some("image/jpeg"));
        assert_eq!(image_type("diffuse.tga"), Some("image/png"));
        assert_eq!(image_type("layer.psd"), Some("image/png"));
        assert_eq!(image_type("albedo.dds"), Some("image/png"));
        assert_eq!(image_type("lightmap.exr"), Some("image/png"));
        assert_eq!(image_type("sky.hdr"), Some("image/png"));
        assert_eq!(image_type("mask.tif"), Some("image/png"));
        assert_eq!(image_type("mask.tiff"), Some("image/png"));
        assert_eq!(image_type("Ship.prefab"), None);
        assert_eq!(image_type("noext"), None);
    }

    #[test]
    fn a_tiny_file_claiming_a_big_image_is_refused_before_allocating() {
        let mut tga = vec![0u8; 26];
        tga[2] = 10;
        tga[12..14].copy_from_slice(&4096u16.to_le_bytes());
        tga[14..16].copy_from_slice(&4096u16.to_le_bytes());
        tga[16] = 32;
        assert!(
            decode_tga_to_png(&tga)
                .unwrap_err()
                .contains("too little data")
        );

        let mut psd = b"8BPS".to_vec();
        psd.extend_from_slice(&1u16.to_be_bytes());
        psd.extend_from_slice(&[0u8; 6]);
        psd.extend_from_slice(&4u16.to_be_bytes());
        psd.extend_from_slice(&4096u32.to_be_bytes());
        psd.extend_from_slice(&4096u32.to_be_bytes());
        psd.extend_from_slice(&8u16.to_be_bytes());
        psd.extend_from_slice(&3u16.to_be_bytes());
        psd.extend_from_slice(&[0u8; 12]);
        psd.extend_from_slice(&1u16.to_be_bytes());
        assert!(
            decode_psd_to_png(&psd)
                .unwrap_err()
                .contains("too little data")
        );
    }

    #[test]
    fn tga_and_psd_decode_to_valid_png() {
        let mut tga = vec![0u8; 18 + 8];
        tga[2] = 2;
        tga[12..14].copy_from_slice(&2u16.to_le_bytes());
        tga[14..16].copy_from_slice(&1u16.to_le_bytes());
        tga[16] = 32;
        tga[17] = 0x20;
        tga[18..22].copy_from_slice(&[0, 0, 255, 255]);
        tga[22..26].copy_from_slice(&[255, 0, 0, 128]);
        let png = decode_tga_to_png(&tga).unwrap();
        assert!(png.starts_with(b"\x89PNG\r\n\x1a\n"));

        let mut psd = Vec::new();
        psd.extend_from_slice(b"8BPS");
        psd.extend_from_slice(&1u16.to_be_bytes());
        psd.extend_from_slice(&[0u8; 6]);
        psd.extend_from_slice(&3u16.to_be_bytes());
        psd.extend_from_slice(&1u32.to_be_bytes());
        psd.extend_from_slice(&2u32.to_be_bytes());
        psd.extend_from_slice(&8u16.to_be_bytes());
        psd.extend_from_slice(&3u16.to_be_bytes());
        psd.extend_from_slice(&0u32.to_be_bytes());
        psd.extend_from_slice(&0u32.to_be_bytes());
        psd.extend_from_slice(&0u32.to_be_bytes());
        psd.extend_from_slice(&0u16.to_be_bytes());
        psd.extend_from_slice(&[255, 0, 0, 255, 0, 0]);
        let psd_png = decode_psd_to_png(&psd).unwrap();
        assert!(psd_png.starts_with(b"\x89PNG\r\n\x1a\n"));

        // 16-bit raw RGB PSD (big-endian u16 samples)
        let mut psd16 = Vec::new();
        psd16.extend_from_slice(b"8BPS");
        psd16.extend_from_slice(&1u16.to_be_bytes());
        psd16.extend_from_slice(&[0u8; 6]);
        psd16.extend_from_slice(&3u16.to_be_bytes());
        psd16.extend_from_slice(&1u32.to_be_bytes());
        psd16.extend_from_slice(&2u32.to_be_bytes());
        psd16.extend_from_slice(&16u16.to_be_bytes());
        psd16.extend_from_slice(&3u16.to_be_bytes());
        psd16.extend_from_slice(&0u32.to_be_bytes());
        psd16.extend_from_slice(&0u32.to_be_bytes());
        psd16.extend_from_slice(&0u32.to_be_bytes());
        psd16.extend_from_slice(&0u16.to_be_bytes());
        psd16.extend_from_slice(&[
            200, 1, 10, 2, // R channel
            40, 3, 220, 4, // G channel
            15, 5, 30, 6, // B channel
        ]);
        let psd16_png = decode_psd_to_png(&psd16).unwrap();
        let decoded16 = image::load_from_memory(&psd16_png).unwrap().to_rgba8();
        assert_eq!(decoded16.dimensions(), (2, 1));
        assert_eq!(decoded16.get_pixel(0, 0).0, [200, 40, 15, 255]);
        assert_eq!(decoded16.get_pixel(1, 0).0, [10, 220, 30, 255]);

        // 16-bit PackBits RGB PSD (1x1 repeated byte 0x80 => 128 on each channel)
        let mut psd16_rle = Vec::new();
        psd16_rle.extend_from_slice(b"8BPS");
        psd16_rle.extend_from_slice(&1u16.to_be_bytes());
        psd16_rle.extend_from_slice(&[0u8; 6]);
        psd16_rle.extend_from_slice(&3u16.to_be_bytes());
        psd16_rle.extend_from_slice(&1u32.to_be_bytes());
        psd16_rle.extend_from_slice(&1u32.to_be_bytes());
        psd16_rle.extend_from_slice(&16u16.to_be_bytes());
        psd16_rle.extend_from_slice(&3u16.to_be_bytes());
        psd16_rle.extend_from_slice(&0u32.to_be_bytes());
        psd16_rle.extend_from_slice(&0u32.to_be_bytes());
        psd16_rle.extend_from_slice(&0u32.to_be_bytes());
        psd16_rle.extend_from_slice(&1u16.to_be_bytes());
        psd16_rle.extend_from_slice(&[0, 3, 0, 3, 0, 3]); // 2-byte scanline length per channel (literal 2 bytes => header 1 + 2 bytes)
        psd16_rle.extend_from_slice(&[1, 180, 0]);
        psd16_rle.extend_from_slice(&[1, 90, 0]);
        psd16_rle.extend_from_slice(&[1, 45, 0]);
        let psd16_rle_png = decode_psd_to_png(&psd16_rle).unwrap();
        let decoded16_rle = image::load_from_memory(&psd16_rle_png).unwrap().to_rgba8();
        assert_eq!(decoded16_rle.get_pixel(0, 0).0, [180, 90, 45, 255]);
    }

    #[test]
    fn dds_exr_hdr_and_tiff_decode_to_valid_png() {
        // 1) Uncompressed RGBA8 DDS (2x1)
        let mut dds_rgba = Dds::new_dxgi(image_dds::ddsfile::NewDxgiParams {
            height: 1,
            width: 2,
            depth: None,
            format: image_dds::ddsfile::DxgiFormat::R8G8B8A8_UNorm,
            mipmap_levels: None,
            array_layers: None,
            caps2: None,
            is_cubemap: false,
            resource_dimension: image_dds::ddsfile::D3D10ResourceDimension::Texture2D,
            alpha_mode: image_dds::ddsfile::AlphaMode::Straight,
        })
        .unwrap();
        dds_rgba.data = vec![255, 0, 0, 255, 0, 255, 0, 128];
        let mut dds_bytes = Vec::new();
        dds_rgba.write(&mut dds_bytes).unwrap();
        let dds_png = decode_dds_to_png(&dds_bytes).unwrap();
        let decoded_dds = image::load_from_memory(&dds_png).unwrap().to_rgba8();
        assert_eq!(decoded_dds.dimensions(), (2, 1));
        assert_eq!(decoded_dds.get_pixel(0, 0).0, [255, 0, 0, 255]);

        // 2) BC1/DXT1 DDS (4x4, 8-byte block)
        let mut dds_bc1 = Dds::new_dxgi(image_dds::ddsfile::NewDxgiParams {
            height: 4,
            width: 4,
            depth: None,
            format: image_dds::ddsfile::DxgiFormat::BC1_UNorm,
            mipmap_levels: None,
            array_layers: None,
            caps2: None,
            is_cubemap: false,
            resource_dimension: image_dds::ddsfile::D3D10ResourceDimension::Texture2D,
            alpha_mode: image_dds::ddsfile::AlphaMode::Opaque,
        })
        .unwrap();
        dds_bc1.data = vec![0x00, 0xf8, 0x00, 0xf8, 0, 0, 0, 0];
        let mut bc1_bytes = Vec::new();
        dds_bc1.write(&mut bc1_bytes).unwrap();
        let bc1_png = decode_dds_to_png(&bc1_bytes).unwrap();
        assert_eq!(image::load_from_memory(&bc1_png).unwrap().width(), 4);

        // 3) EXR & HDR (float32 RGB/RGBA)
        let hdr_img = image::DynamicImage::ImageRgb32F(image::Rgb32FImage::from_pixel(
            2,
            2,
            image::Rgb([2.0, 1.0, 0.25]),
        ));
        let mut exr_bytes = Vec::new();
        hdr_img
            .write_to(&mut Cursor::new(&mut exr_bytes), ImageFormat::OpenExr)
            .unwrap();
        let exr_png = decode_hdr_format_to_png(&exr_bytes, ImageFormat::OpenExr).unwrap();
        assert_eq!(
            image::load_from_memory(&exr_png).unwrap().dimensions(),
            (2, 2)
        );

        let mut hdr_bytes = Vec::new();
        hdr_img
            .write_to(&mut Cursor::new(&mut hdr_bytes), ImageFormat::Hdr)
            .unwrap();
        let hdr_png = decode_hdr_format_to_png(&hdr_bytes, ImageFormat::Hdr).unwrap();
        assert_eq!(
            image::load_from_memory(&hdr_png).unwrap().dimensions(),
            (2, 2)
        );

        // 4) TIFF
        let tiff_img = image::DynamicImage::ImageRgba8(image::RgbaImage::from_pixel(
            3,
            2,
            image::Rgba([12, 34, 56, 255]),
        ));
        let mut tiff_bytes = Vec::new();
        tiff_img
            .write_to(&mut Cursor::new(&mut tiff_bytes), ImageFormat::Tiff)
            .unwrap();
        let tiff_png = decode_tiff_to_png(&tiff_bytes).unwrap();
        let decoded_tiff = image::load_from_memory(&tiff_png).unwrap().to_rgba8();
        assert_eq!(decoded_tiff.dimensions(), (3, 2));
        assert_eq!(decoded_tiff.get_pixel(0, 0).0, [12, 34, 56, 255]);
    }
}
