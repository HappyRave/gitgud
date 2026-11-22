use std::path::Path;

use gix::bstr::BStr;
use serde::Deserialize;

use crate::diff::blob_at;
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
        "tga" | "psd" => "image/png",
        _ => return None,
    })
}

const LFS_POINTER: &[u8] = b"version https://git-lfs.github.com/spec/v1\n";

impl Repo {
    /// The bytes of image `path` at `version`; an LFS pointer is swapped for its
    /// local object.
    ///
    /// # Errors
    /// [`Error::Refused`] when `path` isn't an image, doesn't exist in that
    /// version, or its LFS object isn't downloaded; [`Error::Read`] otherwise.
    pub fn image_bytes(&self, path: &str, version: &Version) -> Result<Vec<u8>, Error> {
        if image_type(path).is_none() {
            return Err(Error::Refused(format!("{path} is not an image")));
        }
        let inside = Path::new(path)
            .components()
            .all(|c| matches!(c, std::path::Component::Normal(_)));
        if !inside {
            return Err(Error::Refused(format!(
                "{path} is not a path inside the repo"
            )));
        }
        let missing = || Error::Refused(format!("{path} is not in that version"));
        let bytes = match version {
            Version::Worktree => match std::fs::read(self.root().join(path)) {
                Ok(d) => d,
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Err(missing()),
                Err(e) => return Err(read_err(e)),
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
                blob_at(&repo, &commit, path)?.ok_or_else(missing)?
            }
            Version::Head => {
                let repo = self.gix();
                match repo.head_commit() {
                    Ok(commit) => blob_at(&repo, &commit, path)?.ok_or_else(missing)?,
                    Err(_) if repo.head().map_err(read_err)?.is_unborn() => return Err(missing()),
                    Err(e) => return Err(read_err(e)),
                }
            }
            Version::Index => {
                let repo = self.gix();
                let index = repo.index_or_empty().map_err(read_err)?;
                match index.entry_by_path(BStr::new(path)) {
                    Some(e) => repo.find_blob(e.id).map_err(read_err)?.data.clone(),
                    None => return Err(missing()),
                }
            }
            Version::Yours | Version::Theirs => {
                let [_, yours, theirs] = self.stages(path)?;
                let picked = if matches!(version, Version::Yours) {
                    yours
                } else {
                    theirs
                };
                picked.ok_or_else(missing)?
            }
        };
        let resolved = self.lfs_resolve(path, bytes)?;
        let ext = Path::new(path)
            .extension()
            .and_then(|e| e.to_str())
            .map(str::to_ascii_lowercase);
        match ext.as_deref() {
            Some("tga") => decode_tga_to_png(&resolved)
                .map_err(|e| Error::Read(format!("{path}: invalid TGA ({e})"))),
            Some("psd") => decode_psd_to_png(&resolved)
                .map_err(|e| Error::Read(format!("{path}: invalid PSD ({e})"))),
            _ => Ok(resolved),
        }
    }

    fn lfs_resolve(&self, path: &str, data: Vec<u8>) -> Result<Vec<u8>, Error> {
        if !data.starts_with(LFS_POINTER) {
            return Ok(data);
        }
        let oid = String::from_utf8_lossy(&data)
            .lines()
            .find_map(|l| l.strip_prefix("oid sha256:").map(str::to_owned))
            .filter(|o| o.len() == 64 && o.bytes().all(|b| b.is_ascii_hexdigit()))
            .ok_or_else(|| Error::Read(format!("{path} has a malformed LFS pointer")))?;
        let object = self
            .gix()
            .common_dir()
            .join("lfs/objects")
            .join(&oid[..2])
            .join(&oid[2..4])
            .join(&oid);
        match std::fs::read(&object) {
            Ok(d) => Ok(d),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Err(Error::Refused(format!(
                "{path}: this LFS version isn't downloaded"
            ))),
            Err(e) => Err(read_err(e)),
        }
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

    Ok(encode_rgba_png(width, height, &rgba))
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
    if depth != 8 || !matches!(color_mode, 1 | 3) || channels == 0 {
        return Err("only 8-bit Grayscale/RGB flat PSDs are supported".into());
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
    let used_channels = channels.min(4);
    let mut planes = vec![vec![255u8; pixels]; 4];

    match compression {
        0 => {
            if pos + pixels * channels > data.len() {
                return Err("truncated raw PSD data".into());
            }
            for (c, plane) in planes.iter_mut().enumerate().take(used_channels) {
                let start = pos + c * pixels;
                plane.copy_from_slice(&data[start..start + pixels]);
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
                        unpack_bits(scan, dst)?;
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
    Ok(encode_rgba_png(width, height, &rgba))
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

fn encode_rgba_png(width: u32, height: u32, rgba: &[u8]) -> Vec<u8> {
    let row_bytes = (width as usize) * 4 + 1;
    let mut raw = Vec::with_capacity(row_bytes * (height as usize));
    for y in 0..height as usize {
        raw.push(0); // filter 0 (None)
        let start = y * (width as usize) * 4;
        raw.extend_from_slice(&rgba[start..start + (width as usize) * 4]);
    }

    // Stored zlib stream (no external deflate dependency needed).
    let mut zlib = Vec::with_capacity(raw.len() + 64);
    zlib.extend_from_slice(&[0x78, 0x01]);
    let mut off = 0usize;
    while off < raw.len() {
        let len = (raw.len() - off).min(65_535);
        let is_last = off + len == raw.len();
        zlib.push(u8::from(is_last));
        let len_u16 = u16::try_from(len).expect("chunk <= 65535");
        zlib.extend_from_slice(&len_u16.to_le_bytes());
        zlib.extend_from_slice(&(!len_u16).to_le_bytes());
        zlib.extend_from_slice(&raw[off..off + len]);
        off += len;
    }
    zlib.extend_from_slice(&adler32(&raw).to_be_bytes());

    let mut png = Vec::with_capacity(zlib.len() + 64);
    png.extend_from_slice(b"\x89PNG\r\n\x1a\n");
    let mut ihdr = Vec::with_capacity(13);
    ihdr.extend_from_slice(&width.to_be_bytes());
    ihdr.extend_from_slice(&height.to_be_bytes());
    ihdr.extend_from_slice(&[8, 6, 0, 0, 0]); // 8-bit RGBA
    write_png_chunk(&mut png, *b"IHDR", &ihdr);
    write_png_chunk(&mut png, *b"IDAT", &zlib);
    write_png_chunk(&mut png, *b"IEND", &[]);
    png
}

fn write_png_chunk(out: &mut Vec<u8>, kind: [u8; 4], data: &[u8]) {
    let len = u32::try_from(data.len()).expect("PNG chunk fits u32");
    out.extend_from_slice(&len.to_be_bytes());
    out.extend_from_slice(&kind);
    out.extend_from_slice(data);
    let mut crc_input = Vec::with_capacity(4 + data.len());
    crc_input.extend_from_slice(&kind);
    crc_input.extend_from_slice(data);
    out.extend_from_slice(&crc32(&crc_input).to_be_bytes());
}

fn adler32(data: &[u8]) -> u32 {
    const MOD: u32 = 65_521;
    let (mut s1, mut s2) = (1u32, 0u32);
    for &b in data {
        s1 = (s1 + u32::from(b)) % MOD;
        s2 = (s2 + s1) % MOD;
    }
    (s2 << 16) | s1
}

fn crc32(data: &[u8]) -> u32 {
    let mut crc = 0xFFFF_FFFFu32;
    for &b in data {
        crc ^= u32::from(b);
        for _ in 0..8 {
            let mask = (crc & 1).wrapping_neg();
            crc = (crc >> 1) ^ (0xEDB8_8320 & mask);
        }
    }
    !crc
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn image_types_by_extension() {
        assert_eq!(image_type("a/b.PNG"), Some("image/png"));
        assert_eq!(image_type("t.jpeg"), Some("image/jpeg"));
        assert_eq!(image_type("diffuse.tga"), Some("image/png"));
        assert_eq!(image_type("layer.psd"), Some("image/png"));
        assert_eq!(image_type("Ship.prefab"), None);
        assert_eq!(image_type("noext"), None);
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
    }
}
