//! A small WAV decoder (no dependencies).
//!
//! A WAV file is a RIFF container: a `RIFF….WAVE` header followed by chunks,
//! each `id (4 bytes) + size (u32 LE) + data`, padded to an even length.
//! We need two of them: `fmt ` (how the audio is stored) and `data` (the
//! audio). Every other chunk (`LIST`, `bext`, `cue `…) is skipped.
//!
//! Supported: PCM 8/16/24/32-bit, IEEE float 32/64-bit, plain or
//! `WAVE_FORMAT_EXTENSIBLE`. Mono and stereo stay as they are; files with more
//! channels are mixed down to mono.

use super::SampleData;

const FORMAT_PCM: u16 = 1;
const FORMAT_FLOAT: u16 = 3;
const FORMAT_EXTENSIBLE: u16 = 0xFFFE;

/// What the `fmt ` chunk tells us.
struct Format {
    tag: u16,
    channels: usize,
    sample_rate: u32,
    /// Bytes per sample frame for ONE channel.
    bytes: usize,
    bits: u16,
}

fn u16_at(b: &[u8], i: usize) -> Option<u16> {
    b.get(i..i + 2).map(|s| u16::from_le_bytes([s[0], s[1]]))
}

fn u32_at(b: &[u8], i: usize) -> Option<u32> {
    b.get(i..i + 4).map(|s| u32::from_le_bytes([s[0], s[1], s[2], s[3]]))
}

fn parse_fmt(c: &[u8]) -> Result<Format, String> {
    let mut tag = u16_at(c, 0).ok_or("fmt chunk too short")?;
    let channels = u16_at(c, 2).ok_or("fmt chunk too short")? as usize;
    let sample_rate = u32_at(c, 4).ok_or("fmt chunk too short")?;
    let block_align = u16_at(c, 12).ok_or("fmt chunk too short")? as usize;
    let bits = u16_at(c, 14).ok_or("fmt chunk too short")?;
    if tag == FORMAT_EXTENSIBLE {
        // the real format is the first two bytes of the sub-format GUID, at offset 24
        tag = u16_at(c, 24).ok_or("extensible fmt chunk too short")?;
    }
    if channels == 0 || sample_rate == 0 {
        return Err("no channels or zero sample rate".into());
    }
    // block_align is the most reliable size (e.g. 20-bit audio in 3-byte slots)
    let bytes = if block_align >= channels && block_align.is_multiple_of(channels) {
        block_align / channels
    } else {
        (bits as usize).div_ceil(8)
    };
    Ok(Format { tag, channels, sample_rate, bytes, bits })
}

/// Turn one stored sample into a float in -1..1.
fn sample_reader(f: &Format) -> Result<fn(&[u8]) -> f32, String> {
    Ok(match (f.tag, f.bytes) {
        (FORMAT_PCM, 1) => |s| (s[0] as f32 - 128.0) / 128.0,
        (FORMAT_PCM, 2) => |s| i16::from_le_bytes([s[0], s[1]]) as f32 / 32768.0,
        (FORMAT_PCM, 3) => |s| (i32::from_le_bytes([0, s[0], s[1], s[2]]) >> 8) as f32 / 8_388_608.0,
        (FORMAT_PCM, 4) => |s| i32::from_le_bytes([s[0], s[1], s[2], s[3]]) as f32 / 2_147_483_648.0,
        (FORMAT_FLOAT, 4) => |s| f32::from_le_bytes([s[0], s[1], s[2], s[3]]),
        (FORMAT_FLOAT, 8) => |s| f64::from_le_bytes([s[0], s[1], s[2], s[3], s[4], s[5], s[6], s[7]]) as f32,
        _ => return Err(format!("unsupported WAV format (tag {}, {} bits)", f.tag, f.bits)),
    })
}

/// Decode a whole WAV file held in memory.
pub fn decode(b: &[u8]) -> Result<SampleData, String> {
    if b.len() < 12 || &b[0..4] != b"RIFF" || &b[8..12] != b"WAVE" {
        return Err("not a RIFF/WAVE file".into());
    }
    let mut fmt = None;
    let mut data: Option<&[u8]> = None;
    let mut i = 12;
    while i + 8 <= b.len() {
        let id = &b[i..i + 4];
        let size = u32_at(b, i + 4).unwrap_or(0) as usize;
        let start = i + 8;
        // a truncated last chunk (or a streaming size of 0xFFFFFFFF): take what's there
        let end = start.saturating_add(size).min(b.len());
        match id {
            b"fmt " => fmt = Some(parse_fmt(&b[start..end])?),
            b"data" => data = Some(&b[start..end]),
            _ => {}
        }
        // chunks are padded to an even number of bytes
        i = start.saturating_add(size).saturating_add(size & 1);
    }
    let fmt = fmt.ok_or("no fmt chunk")?;
    let data = data.ok_or("no data chunk")?;
    let read = sample_reader(&fmt)?;

    let frame = fmt.bytes * fmt.channels;
    let frames = data.len() / frame;
    let keep = if fmt.channels == 2 { 2 } else { 1 };
    let mut channels = vec![Vec::with_capacity(frames); keep];
    for f in data.chunks_exact(frame) {
        if fmt.channels <= 2 {
            for (c, out) in channels.iter_mut().enumerate() {
                let x = read(&f[c * fmt.bytes..]);
                out.push(if x.is_finite() { x } else { 0.0 });
            }
        } else {
            // downmix: the average of every channel
            let sum: f32 = f.chunks_exact(fmt.bytes).map(read).filter(|x| x.is_finite()).sum();
            channels[0].push(sum / fmt.channels as f32);
        }
    }
    Ok(SampleData { sample_rate: fmt.sample_rate as f32, channels })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Build a WAV in memory. `samples` are already-encoded little-endian bytes.
    fn build_wav(tag: u16, channels: u16, rate: u32, bits: u16, samples: &[u8], extensible: bool, extra_chunk: bool) -> Vec<u8> {
        let block = channels * bits.div_ceil(8);
        let mut fmt = Vec::new();
        fmt.extend_from_slice(&(if extensible { FORMAT_EXTENSIBLE } else { tag }).to_le_bytes());
        fmt.extend_from_slice(&channels.to_le_bytes());
        fmt.extend_from_slice(&rate.to_le_bytes());
        fmt.extend_from_slice(&(rate * block as u32).to_le_bytes());
        fmt.extend_from_slice(&block.to_le_bytes());
        fmt.extend_from_slice(&bits.to_le_bytes());
        if extensible {
            fmt.extend_from_slice(&22u16.to_le_bytes()); // cbSize
            fmt.extend_from_slice(&bits.to_le_bytes()); // valid bits
            fmt.extend_from_slice(&0u32.to_le_bytes()); // channel mask
            fmt.extend_from_slice(&tag.to_le_bytes()); // sub-format GUID…
            fmt.extend_from_slice(&[0, 0, 0, 0, 0x10, 0, 0x80, 0, 0, 0xAA, 0, 0x38, 0x9B, 0x71]);
        }
        let mut body = Vec::new();
        body.extend_from_slice(b"WAVE");
        if extra_chunk {
            // an unknown chunk with an odd size → one pad byte
            body.extend_from_slice(b"LIST");
            body.extend_from_slice(&3u32.to_le_bytes());
            body.extend_from_slice(&[1, 2, 3, 0]);
        }
        body.extend_from_slice(b"fmt ");
        body.extend_from_slice(&(fmt.len() as u32).to_le_bytes());
        body.extend_from_slice(&fmt);
        body.extend_from_slice(b"data");
        body.extend_from_slice(&(samples.len() as u32).to_le_bytes());
        body.extend_from_slice(samples);
        let mut out = b"RIFF".to_vec();
        out.extend_from_slice(&(body.len() as u32).to_le_bytes());
        out.extend_from_slice(&body);
        out
    }

    fn close(a: &[f32], b: &[f32]) -> bool {
        a.len() == b.len() && a.iter().zip(b).all(|(x, y)| (x - y).abs() < 1e-3)
    }

    const WANT: [f32; 3] = [0.0, 0.5, -0.5];

    #[test]
    fn pcm8() {
        let d = decode(&build_wav(1, 1, 8000, 8, &[128, 192, 64], false, false)).unwrap();
        assert_eq!(d.sample_rate, 8000.0);
        assert!(close(&d.channels[0], &WANT));
    }

    #[test]
    fn pcm16_with_unknown_odd_chunk() {
        let bytes: Vec<u8> = [0i16, 16384, -16384].iter().flat_map(|s| s.to_le_bytes()).collect();
        let d = decode(&build_wav(1, 1, 44100, 16, &bytes, false, true)).unwrap();
        assert_eq!(d.channels.len(), 1);
        assert!(close(&d.channels[0], &WANT));
    }

    #[test]
    fn pcm24() {
        let bytes: Vec<u8> = [0i32, 4_194_304, -4_194_304].iter().flat_map(|s| s.to_le_bytes()[..3].to_vec()).collect();
        let d = decode(&build_wav(1, 1, 48000, 24, &bytes, false, false)).unwrap();
        assert!(close(&d.channels[0], &WANT));
    }

    #[test]
    fn pcm32_extensible() {
        let bytes: Vec<u8> = [0i32, 1 << 30, -(1 << 30)].iter().flat_map(|s| s.to_le_bytes()).collect();
        let d = decode(&build_wav(1, 1, 96000, 32, &bytes, true, false)).unwrap();
        assert!(close(&d.channels[0], &WANT));
    }

    #[test]
    fn float32_stereo() {
        let bytes: Vec<u8> = [0.0f32, 1.0, 0.5, -0.5, -0.5, 0.25].iter().flat_map(|s| s.to_le_bytes()).collect();
        let d = decode(&build_wav(3, 2, 44100, 32, &bytes, false, true)).unwrap();
        assert_eq!(d.channels.len(), 2);
        assert!(close(&d.channels[0], &WANT));
        assert!(close(&d.channels[1], &[1.0, -0.5, 0.25]));
    }

    #[test]
    fn float64_extensible() {
        let bytes: Vec<u8> = WANT.iter().flat_map(|s| (*s as f64).to_le_bytes()).collect();
        let d = decode(&build_wav(3, 1, 22050, 64, &bytes, true, false)).unwrap();
        assert!(close(&d.channels[0], &WANT));
    }

    #[test]
    fn many_channels_mix_to_mono() {
        let bytes: Vec<u8> = [1000i16, 3000, 2000, 2000].iter().flat_map(|s| s.to_le_bytes()).collect();
        let d = decode(&build_wav(1, 4, 44100, 16, &bytes, false, false)).unwrap();
        assert_eq!(d.channels.len(), 1);
        assert!((d.channels[0][0] - 2000.0 / 32768.0).abs() < 1e-5);
    }

    #[test]
    fn rejects_garbage_and_truncation_is_safe() {
        assert!(decode(b"not a wav at all").is_err());
        let bytes: Vec<u8> = [0i16, 16384, -16384].iter().flat_map(|s| s.to_le_bytes()).collect();
        let mut w = build_wav(1, 1, 44100, 16, &bytes, false, false);
        w.truncate(w.len() - 3); // cut mid-sample
        let d = decode(&w).unwrap();
        assert_eq!(d.channels[0].len(), 1);
    }
}
