use std::io::Cursor;

use ebur128::{EbuR128, Mode};
use serde::Serialize;
use symphonia::core::codecs::audio::AudioDecoderOptions;
use symphonia::core::errors::Error as DecodeError;
use symphonia::core::formats::probe::Hint;
use symphonia::core::formats::{FormatOptions, TrackType};
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;

/// How many min / max pairs the waveform keeps per channel.
pub const WAVE_BUCKETS: usize = 1600;
/// Frames folded into one min / max pair while decoding, before the final buckets.
const BLOCK: usize = 256;

/// What the audio viewer shows about one version of a sound file.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AudioInfo {
    /// The container, in the decoder's words ("Waveform Audio File Format").
    pub format: String,
    pub codec: String,
    /// The file's own rate, not the webview's playback rate.
    pub sample_rate: u32,
    pub channels: u16,
    pub bits_per_sample: Option<u32>,
    pub frames: u64,
    pub duration: f64,
    /// Highest sample in dBFS; `None` for silence.
    pub peak_dbfs: Option<f64>,
    /// Integrated loudness (EBU R128) in LUFS; `None` when too short or silent.
    pub loudness_lufs: Option<f64>,
    /// Per channel, [`WAVE_BUCKETS`] (or fewer) `[min, max]` pairs.
    pub peaks: Vec<Vec<[f32; 2]>>,
}

/// Decoded interleaved samples, for re-encoding.
struct Decoded {
    info: AudioInfo,
    samples: Option<Vec<f32>>,
}

/// Decodes a whole sound file (WAV, AIFF, MP3, Ogg Vorbis, FLAC, ALAC, AAC / M4A, CAF).
///
/// # Errors
/// A plain-words reason when the bytes can't be decoded.
pub fn read_audio(bytes: Vec<u8>, ext: &str) -> Result<AudioInfo, String> {
    decode(bytes, ext, false).map(|d| d.info)
}

/// The file decoded to 16-bit PCM WAV, for formats the webview can't play (AIFF).
///
/// # Errors
/// As [`read_audio`].
pub fn to_wav(bytes: Vec<u8>, ext: &str) -> Result<Vec<u8>, String> {
    let d = decode(bytes, ext, true)?;
    let samples = d.samples.unwrap_or_default();
    let ch = d.info.channels.max(1);
    let data_len =
        u32::try_from(samples.len() * 2).map_err(|_| "too long to convert".to_owned())?;
    let mut out = Vec::with_capacity(44 + samples.len() * 2);
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&(36 + data_len).to_le_bytes());
    out.extend_from_slice(b"WAVEfmt ");
    out.extend_from_slice(&16u32.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&ch.to_le_bytes());
    out.extend_from_slice(&d.info.sample_rate.to_le_bytes());
    out.extend_from_slice(&(d.info.sample_rate * u32::from(ch) * 2).to_le_bytes());
    out.extend_from_slice(&(ch * 2).to_le_bytes());
    out.extend_from_slice(&16u16.to_le_bytes());
    out.extend_from_slice(b"data");
    out.extend_from_slice(&data_len.to_le_bytes());
    for s in samples {
        out.extend_from_slice(&((s.clamp(-1.0, 1.0) * 32767.0).round() as i16).to_le_bytes());
    }
    Ok(out)
}

fn decode(bytes: Vec<u8>, ext: &str, keep: bool) -> Result<Decoded, String> {
    let unreadable = |e: DecodeError| format!("not a sound file this viewer can read ({e})");
    let mss = MediaSourceStream::new(Box::new(Cursor::new(bytes)), Default::default());
    let mut hint = Hint::new();
    hint.with_extension(ext);
    let mut format = symphonia::default::get_probe()
        .probe(
            &hint,
            mss,
            FormatOptions::default(),
            MetadataOptions::default(),
        )
        .map_err(unreadable)?;
    let container = format.format_info().long_name.to_owned();
    let track = format
        .default_track(TrackType::Audio)
        .ok_or("no audio track in this file")?;
    let track_id = track.id;
    let params = track
        .codec_params
        .as_ref()
        .and_then(|p| p.audio())
        .ok_or("no audio track in this file")?
        .clone();
    let codecs = symphonia::default::get_codecs();
    let codec = codecs.get_audio_decoder(params.codec).map_or_else(
        || "unknown".to_owned(),
        |d| d.codec.info.long_name.to_owned(),
    );
    let mut decoder = codecs
        .make_audio_decoder(&params, &AudioDecoderOptions::default())
        .map_err(unreadable)?;

    let mut rate = params.sample_rate.unwrap_or(0);
    let mut channels = params.channels.as_ref().map_or(0, |c| c.count());
    let mut meter: Option<EbuR128> = None;
    let mut blocks: Vec<Vec<[f32; 2]>> = Vec::new();
    let mut in_block = 0usize;
    let mut frames = 0u64;
    let mut scratch: Vec<f32> = Vec::new();
    let mut kept = keep.then(Vec::new);
    loop {
        let packet = match format.next_packet() {
            Ok(Some(p)) => p,
            Ok(None) => break,
            Err(DecodeError::ResetRequired) => break,
            Err(e) => return Err(unreadable(e)),
        };
        if packet.track_id != track_id {
            continue;
        }
        let buf = match decoder.decode(&packet) {
            Ok(b) => b,
            Err(DecodeError::DecodeError(_)) => continue,
            Err(e) => return Err(unreadable(e)),
        };
        if meter.is_none() {
            rate = buf.spec().rate();
            channels = buf.spec().channels().count();
            meter = EbuR128::new(channels as u32, rate, Mode::I | Mode::SAMPLE_PEAK).ok();
            blocks = vec![Vec::new(); channels];
        }
        scratch.resize(buf.samples_interleaved(), 0.0);
        buf.copy_to_slice_interleaved(&mut scratch);
        if let Some(m) = meter.as_mut() {
            m.add_frames_f32(&scratch)
                .map_err(|e| format!("can't measure loudness ({e})"))?;
        }
        for frame in scratch.chunks_exact(channels.max(1)) {
            if in_block == 0 {
                for b in &mut blocks {
                    b.push([f32::MAX, f32::MIN]);
                }
            }
            for (c, s) in frame.iter().enumerate() {
                let p = blocks[c].last_mut().expect("pushed above");
                p[0] = p[0].min(*s);
                p[1] = p[1].max(*s);
            }
            in_block = (in_block + 1) % BLOCK;
        }
        frames += (scratch.len() / channels.max(1)) as u64;
        if let Some(k) = kept.as_mut() {
            k.extend_from_slice(&scratch);
        }
    }
    if rate == 0 {
        return Err("this file has no sound in it".into());
    }
    let peak = meter.as_ref().map_or(0.0, |m| {
        (0..channels as u32)
            .filter_map(|c| m.sample_peak(c).ok())
            .fold(0.0, f64::max)
    });
    let loudness = meter
        .as_ref()
        .and_then(|m| m.loudness_global().ok())
        .filter(|l| l.is_finite());
    Ok(Decoded {
        info: AudioInfo {
            format: container,
            codec,
            sample_rate: rate,
            channels: channels as u16,
            bits_per_sample: params.bits_per_sample,
            frames,
            duration: frames as f64 / f64::from(rate),
            peak_dbfs: (peak > 0.0).then(|| 20.0 * peak.log10()),
            loudness_lufs: loudness,
            peaks: blocks.iter().map(|b| bucket(b)).collect(),
        },
        samples: kept,
    })
}

/// Folds per-block min / max pairs into at most [`WAVE_BUCKETS`].
fn bucket(blocks: &[[f32; 2]]) -> Vec<[f32; 2]> {
    let n = blocks.len().min(WAVE_BUCKETS);
    (0..n)
        .map(|i| {
            let (a, b) = (
                i * blocks.len() / n,
                ((i + 1) * blocks.len() / n).max(i * blocks.len() / n + 1),
            );
            blocks[a..b].iter().fold([f32::MAX, f32::MIN], |acc, p| {
                [acc[0].min(p[0]), acc[1].max(p[1])]
            })
        })
        .collect()
}

/// A 16-bit PCM WAV of `channels` sine waves, for tests.
#[cfg(any(test, feature = "test-util"))]
#[must_use]
pub fn sine_wav(rate: u32, channels: u16, seconds: f32, amplitude: f32) -> Vec<u8> {
    let frames = (rate as f32 * seconds) as usize;
    let mut samples = Vec::with_capacity(frames * usize::from(channels));
    for i in 0..frames {
        let s = amplitude * (i as f32 * 440.0 * std::f32::consts::TAU / rate as f32).sin();
        for _ in 0..channels {
            samples.push(s);
        }
    }
    let data_len = (samples.len() * 2) as u32;
    let mut out = b"RIFF".to_vec();
    out.extend_from_slice(&(36 + data_len).to_le_bytes());
    out.extend_from_slice(b"WAVEfmt ");
    out.extend_from_slice(&16u32.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&channels.to_le_bytes());
    out.extend_from_slice(&rate.to_le_bytes());
    out.extend_from_slice(&(rate * u32::from(channels) * 2).to_le_bytes());
    out.extend_from_slice(&(channels * 2).to_le_bytes());
    out.extend_from_slice(&16u16.to_le_bytes());
    out.extend_from_slice(b"data");
    out.extend_from_slice(&data_len.to_le_bytes());
    for s in samples {
        out.extend_from_slice(&((s * 32767.0).round() as i16).to_le_bytes());
    }
    out
}

/// The same tone as a 16-bit big-endian AIFF, for tests.
#[cfg(test)]
fn sine_aiff(rate: u32, seconds: f32) -> Vec<u8> {
    let wav = sine_wav(rate, 1, seconds, 0.5);
    let pcm: Vec<u8> = wav[44..]
        .as_chunks::<2>()
        .0
        .iter()
        .flat_map(|[lo, hi]| [*hi, *lo])
        .collect();
    let frames = (pcm.len() / 2) as u32;
    // 80-bit extended float for the rate: exponent 16383 + 15 for 44100..65535.
    let mut ext = [0u8; 10];
    let exp = 16383 + 31 - rate.leading_zeros() as u16;
    ext[0..2].copy_from_slice(&exp.to_be_bytes());
    ext[2..6].copy_from_slice(&(rate << rate.leading_zeros()).to_be_bytes());
    let mut comm = Vec::new();
    comm.extend_from_slice(&1u16.to_be_bytes());
    comm.extend_from_slice(&frames.to_be_bytes());
    comm.extend_from_slice(&16u16.to_be_bytes());
    comm.extend_from_slice(&ext);
    let mut out = b"FORM".to_vec();
    let body_len = 4 + 8 + comm.len() + 8 + 8 + pcm.len();
    out.extend_from_slice(&(body_len as u32).to_be_bytes());
    out.extend_from_slice(b"AIFF");
    out.extend_from_slice(b"COMM");
    out.extend_from_slice(&(comm.len() as u32).to_be_bytes());
    out.extend(comm);
    out.extend_from_slice(b"SSND");
    out.extend_from_slice(&((8 + pcm.len()) as u32).to_be_bytes());
    out.extend_from_slice(&[0; 8]);
    out.extend(pcm);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_rate_length_peak_loudness_and_a_waveform() {
        let info = read_audio(sine_wav(48_000, 2, 2.0, 0.5), "wav").unwrap();
        assert_eq!(
            (info.format.as_str(), info.sample_rate, info.channels),
            ("Waveform Audio File Format", 48_000, 2)
        );
        assert_eq!(info.frames, 96_000);
        assert!((info.duration - 2.0).abs() < 1e-9);
        assert!(
            (info.peak_dbfs.unwrap() - -6.02).abs() < 0.05,
            "{:?}",
            info.peak_dbfs
        );
        let lufs = info.loudness_lufs.unwrap();
        assert!((-12.0..-6.0).contains(&lufs), "{lufs}");
        assert_eq!(info.peaks.len(), 2);
        assert_eq!(info.peaks[0].len(), 96_000usize.div_ceil(BLOCK));
        assert!(info.peaks[0].iter().all(|p| p[0] >= -0.51 && p[1] <= 0.51));

        let quiet = read_audio(sine_wav(44_100, 1, 2.0, 0.25), "wav").unwrap();
        assert!(quiet.loudness_lufs.unwrap() < lufs - 5.0);
        assert!(read_audio(b"not audio at all".to_vec(), "wav").is_err());
    }

    #[test]
    fn long_files_fold_into_the_bucket_count() {
        let info = read_audio(sine_wav(8_000, 1, 60.0, 0.9), "wav").unwrap();
        assert_eq!(info.peaks[0].len(), WAVE_BUCKETS);
    }

    #[test]
    fn decodes_aiff_and_reencodes_it_as_wav() {
        let aiff = sine_aiff(44_100, 1.0);
        let info = read_audio(aiff.clone(), "aiff").unwrap();
        assert_eq!(
            (info.format.as_str(), info.sample_rate, info.channels),
            ("Audio Interchange File Format", 44_100, 1)
        );
        assert_eq!(info.frames, 44_100);
        let wav = to_wav(aiff, "aiff").unwrap();
        let back = read_audio(wav, "wav").unwrap();
        assert_eq!((back.sample_rate, back.frames), (44_100, 44_100));
        assert!((back.peak_dbfs.unwrap() - info.peak_dbfs.unwrap()).abs() < 0.01);
    }
}
