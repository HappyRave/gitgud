use serde::Serialize;

/// What the video viewer shows about one version of an MP4 / MOV file, read from
/// its boxes without decoding.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VideoInfo {
    pub codec: String,
    pub width: u32,
    pub height: u32,
    pub duration: f64,
    pub fps: Option<f64>,
    pub frames: Option<u64>,
    /// Whole-file bits per second.
    pub bitrate: Option<f64>,
    pub audio_codec: Option<String>,
}

#[derive(Default)]
struct Track {
    handler: [u8; 4],
    codec: [u8; 4],
    width: u32,
    height: u32,
    timescale: u32,
    duration: u64,
    samples: u64,
}

/// Reads an MP4 / MOV (ISO base media) file's video and audio tracks.
///
/// # Errors
/// A plain-words reason when it isn't one, or has no video track.
pub fn read_video(bytes: &[u8]) -> Result<VideoInfo, String> {
    let moov = boxes(bytes)
        .find(|(t, _)| t == b"moov")
        .map(|(_, b)| b)
        .ok_or(NO_MOOV)?;
    read_moov(moov, bytes.len() as u64)
}

const NO_MOOV: &str = "not an MP4 / MOV file this viewer can read (no moov box)";
/// The largest `moov` box read: a movie's index, megabytes at most in practice.
const MAX_MOOV: u64 = 256 << 20;

/// As [`read_video`], from a file on disk: only the top-level box headers and the
/// `moov` box are read, not the media data (gigabytes for a long video).
///
/// # Errors
/// As [`read_video`], or when the file can't be read.
pub fn read_video_file(path: &std::path::Path) -> Result<VideoInfo, String> {
    use std::io::{Read, Seek, SeekFrom};
    let mut file = std::fs::File::open(path).map_err(|e| e.to_string())?;
    let len = file.metadata().map_err(|e| e.to_string())?.len();
    let mut pos = 0u64;
    while pos + 8 <= len {
        file.seek(SeekFrom::Start(pos)).map_err(|e| e.to_string())?;
        let mut head = [0u8; 16];
        file.read_exact(&mut head[..8]).map_err(|e| e.to_string())?;
        let (size, header) = match u32::from_be_bytes([head[0], head[1], head[2], head[3]]) {
            0 => (len - pos, 8),
            1 => {
                file.read_exact(&mut head[8..]).map_err(|e| e.to_string())?;
                let large = u64::from_be_bytes(head[8..16].try_into().map_err(|_| NO_MOOV)?);
                (large, 16)
            }
            n => (u64::from(n), 8),
        };
        if size < header || pos + size > len {
            return Err(NO_MOOV.into());
        }
        if &head[4..8] == b"moov" {
            let body_len = size - header;
            if body_len > MAX_MOOV {
                return Err("its movie index is too big to read".into());
            }
            let mut moov = vec![0u8; usize::try_from(body_len).map_err(|e| e.to_string())?];
            file.read_exact(&mut moov).map_err(|e| e.to_string())?;
            return read_moov(&moov, len);
        }
        pos += size;
    }
    Err(NO_MOOV.into())
}

/// The tracks in `moov`, the bitrate over the whole file's `file_len` bytes.
fn read_moov(moov: &[u8], file_len: u64) -> Result<VideoInfo, String> {
    let mut movie_duration = 0.0;
    let mut tracks = Vec::new();
    for (t, body) in boxes(moov) {
        match &t {
            b"mvhd" => {
                let (scale, dur) = header_times(body, 12, 16, 20, 24);
                if scale > 0 {
                    movie_duration = dur as f64 / f64::from(scale);
                }
            }
            b"trak" => tracks.push(track(body)),
            _ => {}
        }
    }
    let video = tracks
        .iter()
        .find(|t| &t.handler == b"vide")
        .ok_or("this file has no video track")?;
    let seconds = if video.timescale > 0 && video.duration > 0 {
        video.duration as f64 / f64::from(video.timescale)
    } else {
        movie_duration
    };
    let fps = (seconds > 0.0 && video.samples > 0).then(|| video.samples as f64 / seconds);
    Ok(VideoInfo {
        codec: codec_name(&video.codec),
        width: video.width,
        height: video.height,
        duration: if seconds > 0.0 {
            seconds
        } else {
            movie_duration
        },
        fps,
        frames: (video.samples > 0).then_some(video.samples),
        bitrate: (seconds > 0.0).then(|| file_len as f64 * 8.0 / seconds),
        audio_codec: tracks
            .iter()
            .find(|t| &t.handler == b"soun")
            .map(|t| codec_name(&t.codec)),
    })
}

fn track(body: &[u8]) -> Track {
    let mut t = Track::default();
    for (kind, b) in boxes(body) {
        match &kind {
            b"tkhd" if b.len() >= 8 => {
                let n = b.len();
                t.width = be32(b, n - 8).unwrap_or(0) >> 16;
                t.height = be32(b, n - 4).unwrap_or(0) >> 16;
            }
            b"mdia" => {
                for (k, m) in boxes(b) {
                    match &k {
                        b"mdhd" => (t.timescale, t.duration) = header_times(m, 12, 16, 20, 24),
                        b"hdlr" => {
                            if let Some(h) = m.get(8..12) {
                                t.handler.copy_from_slice(h);
                            }
                        }
                        b"minf" => {
                            for (_, stbl) in boxes(m).filter(|(k, _)| k == b"stbl") {
                                for (k, s) in boxes(stbl) {
                                    match &k {
                                        b"stsd" => {
                                            if let Some(c) = s.get(12..16) {
                                                t.codec.copy_from_slice(c);
                                            }
                                        }
                                        b"stts" => {
                                            let n = be32(s, 4).unwrap_or(0) as usize;
                                            t.samples = (0..n)
                                                .map_while(|i| be32(s, 8 + i * 8))
                                                .map(u64::from)
                                                .sum();
                                        }
                                        _ => {}
                                    }
                                }
                            }
                        }
                        _ => {}
                    }
                }
            }
            _ => {}
        }
    }
    t
}

/// Timescale and duration of an `mvhd` / `mdhd`: version 0 has 32-bit fields at
/// `scale0` / `dur0`, version 1 64-bit times with the scale at `scale1`, duration at `dur1`.
fn header_times(b: &[u8], scale0: usize, dur0: usize, scale1: usize, dur1: usize) -> (u32, u64) {
    if b.first() == Some(&1) {
        (be32(b, scale1).unwrap_or(0), be64(b, dur1).unwrap_or(0))
    } else {
        (
            be32(b, scale0).unwrap_or(0),
            u64::from(be32(b, dur0).unwrap_or(0)),
        )
    }
}

fn codec_name(fourcc: &[u8; 4]) -> String {
    match fourcc {
        b"avc1" | b"avc3" => "H.264".into(),
        b"hvc1" | b"hev1" => "HEVC".into(),
        b"av01" => "AV1".into(),
        b"vp09" => "VP9".into(),
        b"vp08" => "VP8".into(),
        b"mp4v" => "MPEG-4 Visual".into(),
        b"apch" | b"apcn" | b"apcs" | b"apco" | b"ap4h" | b"ap4x" => "ProRes".into(),
        b"mp4a" => "AAC".into(),
        b"Opus" => "Opus".into(),
        b"ac-3" => "AC-3".into(),
        b"lpcm" | b"sowt" | b"twos" => "PCM".into(),
        [0, 0, 0, 0] => "unknown".into(),
        other => String::from_utf8_lossy(other).trim().to_owned(),
    }
}

fn be32(b: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_be_bytes(b.get(at..at + 4)?.try_into().ok()?))
}

fn be64(b: &[u8], at: usize) -> Option<u64> {
    Some(u64::from_be_bytes(b.get(at..at + 8)?.try_into().ok()?))
}

/// The boxes directly inside `b`: `(type, body)`, stopping at the first broken one.
fn boxes(b: &[u8]) -> impl Iterator<Item = ([u8; 4], &[u8])> {
    let mut at = 0usize;
    std::iter::from_fn(move || {
        let size = be32(b, at)? as usize;
        let kind: [u8; 4] = b.get(at + 4..at + 8)?.try_into().ok()?;
        let (start, end) = match size {
            0 => (at + 8, b.len()),
            1 => (
                at + 16,
                at.checked_add(usize::try_from(be64(b, at + 8)?).ok()?)?,
            ),
            n if n >= 8 => (at + 8, at.checked_add(n)?),
            _ => return None,
        };
        let body = b.get(start..end)?;
        at = end;
        Some((kind, body))
    })
}

/// A minimal MP4 with one video track (and optionally an audio track), for tests.
#[cfg(any(test, feature = "test-util"))]
#[must_use]
pub fn tiny_mp4(codec: &[u8; 4], width: u32, height: u32, fps: u32, seconds: u32) -> Vec<u8> {
    fn bx(kind: &[u8; 4], body: &[u8]) -> Vec<u8> {
        let mut out = ((body.len() + 8) as u32).to_be_bytes().to_vec();
        out.extend_from_slice(kind);
        out.extend_from_slice(body);
        out
    }
    let scale = 90_000u32;
    let mut mvhd = vec![0u8; 100];
    mvhd[12..16].copy_from_slice(&1000u32.to_be_bytes());
    mvhd[16..20].copy_from_slice(&(seconds * 1000).to_be_bytes());
    let mut tkhd = vec![0u8; 84];
    tkhd[76..80].copy_from_slice(&(width << 16).to_be_bytes());
    tkhd[80..84].copy_from_slice(&(height << 16).to_be_bytes());
    let mut mdhd = vec![0u8; 24];
    mdhd[12..16].copy_from_slice(&scale.to_be_bytes());
    mdhd[16..20].copy_from_slice(&(scale * seconds).to_be_bytes());
    let mut hdlr = vec![0u8; 24];
    hdlr[8..12].copy_from_slice(b"vide");
    let mut stsd = vec![0u8; 8];
    stsd[4..8].copy_from_slice(&1u32.to_be_bytes());
    stsd.extend_from_slice(&86u32.to_be_bytes());
    stsd.extend_from_slice(codec);
    stsd.extend_from_slice(&[0; 78]);
    let mut stts = vec![0u8; 4];
    stts.extend_from_slice(&1u32.to_be_bytes());
    stts.extend_from_slice(&(fps * seconds).to_be_bytes());
    stts.extend_from_slice(&(scale / fps).to_be_bytes());
    let stbl = bx(b"stbl", &[bx(b"stsd", &stsd), bx(b"stts", &stts)].concat());
    let minf = bx(b"minf", &stbl);
    let mdia = bx(
        b"mdia",
        &[bx(b"mdhd", &mdhd), bx(b"hdlr", &hdlr), minf].concat(),
    );
    let trak = bx(b"trak", &[bx(b"tkhd", &tkhd), mdia].concat());
    let moov = bx(b"moov", &[bx(b"mvhd", &mvhd), trak].concat());
    [
        bx(b"ftyp", b"isom\0\0\0\0isom"),
        moov,
        bx(b"mdat", &[0; 64]),
    ]
    .concat()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_a_file_by_its_boxes_past_the_media_data() {
        let movie = tiny_mp4(b"avc1", 1280, 720, 25, 4);
        let media = vec![7u8; 3 << 20];
        let mut file = Vec::new();
        // A 64-bit sized mdat before the movie index, as recorders write it.
        file.extend_from_slice(&1u32.to_be_bytes());
        file.extend_from_slice(b"mdat");
        file.extend_from_slice(&(16 + media.len() as u64).to_be_bytes());
        file.extend_from_slice(&media);
        file.extend_from_slice(&movie);
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("clip.mp4");
        std::fs::write(&path, &file).unwrap();

        let v = read_video_file(&path).unwrap();

        assert_eq!(v, read_video(&file).unwrap());
        assert_eq!((v.width, v.height), (1280, 720));
        std::fs::write(&path, &file[..16 + media.len() + 20]).unwrap();
        assert!(read_video_file(&path).is_err(), "cut short");
    }

    #[test]
    fn reads_codec_size_fps_and_bitrate() {
        let file = tiny_mp4(b"avc1", 1920, 1080, 30, 8);
        let v = read_video(&file).unwrap();
        assert_eq!(v.codec, "H.264");
        assert_eq!((v.width, v.height), (1920, 1080));
        assert!((v.duration - 8.0).abs() < 1e-9);
        assert!((v.fps.unwrap() - 30.0).abs() < 1e-9);
        assert_eq!(v.frames, Some(240));
        assert!((v.bitrate.unwrap() - file.len() as f64).abs() < 1e-9);
        assert_eq!(v.audio_codec, None);
        assert_eq!(
            read_video(&tiny_mp4(b"hvc1", 640, 360, 24, 2))
                .unwrap()
                .codec,
            "HEVC"
        );
    }

    #[test]
    fn refuses_what_isnt_an_mp4() {
        assert!(read_video(b"not a video").is_err());
        assert!(read_video(&[0, 0, 0, 4, b'm', b'o', b'o', b'v']).is_err());
    }
}
