//! Writing WAV files: 16-bit (with TPDF dither), 24-bit (the studio default)
//! or 32-bit float (no clipping at all, for further mixing).

use std::sync::atomic::{AtomicU8, Ordering};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WavFormat {
    /// CD quality; TPDF dither hides the rounding noise.
    Pcm16,
    /// The studio standard (the default).
    Pcm24,
    /// 32-bit float: keeps peaks above 0 dBFS, for a DAW.
    Float32,
}

static DEFAULT: AtomicU8 = AtomicU8::new(1);

impl WavFormat {
    pub fn from_name(s: &str) -> Option<WavFormat> {
        match s.trim().trim_start_matches('-').to_lowercase().as_str() {
            "16" | "16bit" | "cd" => Some(WavFormat::Pcm16),
            "24" | "24bit" => Some(WavFormat::Pcm24),
            "32f" | "32" | "float" | "32bit" => Some(WavFormat::Float32),
            _ => None,
        }
    }

    pub fn name(self) -> &'static str {
        match self { WavFormat::Pcm16 => "16 bits", WavFormat::Pcm24 => "24 bits", WavFormat::Float32 => "32 bits flottant" }
    }

    fn bytes(self) -> u16 { match self { WavFormat::Pcm16 => 2, WavFormat::Pcm24 => 3, WavFormat::Float32 => 4 } }

    /// The format every export uses unless told otherwise: `$LOOM_WAV`
    /// (16, 24 or 32f), else what [`WavFormat::set_default`] chose, else 24 bits.
    pub fn current() -> WavFormat {
        if let Some(f) = std::env::var("LOOM_WAV").ok().as_deref().and_then(WavFormat::from_name) { return f; }
        match DEFAULT.load(Ordering::Relaxed) { 0 => WavFormat::Pcm16, 2 => WavFormat::Float32, _ => WavFormat::Pcm24 }
    }

    pub fn set_default(f: WavFormat) {
        DEFAULT.store(match f { WavFormat::Pcm16 => 0, WavFormat::Pcm24 => 1, WavFormat::Float32 => 2 }, Ordering::Relaxed);
    }

    /// The 44-byte header for `data_len` bytes of interleaved stereo.
    pub fn header(self, sample_rate: u32, data_len: u32) -> Vec<u8> {
        let block = 2 * self.bytes();
        let mut h = Vec::with_capacity(44);
        h.extend_from_slice(b"RIFF");
        h.extend_from_slice(&(36 + data_len).to_le_bytes());
        h.extend_from_slice(b"WAVEfmt ");
        h.extend_from_slice(&16u32.to_le_bytes());
        h.extend_from_slice(&(if self == WavFormat::Float32 { 3u16 } else { 1u16 }).to_le_bytes());
        h.extend_from_slice(&2u16.to_le_bytes());
        h.extend_from_slice(&sample_rate.to_le_bytes());
        h.extend_from_slice(&(sample_rate * block as u32).to_le_bytes());
        h.extend_from_slice(&block.to_le_bytes());
        h.extend_from_slice(&(self.bytes() * 8).to_le_bytes());
        h.extend_from_slice(b"data");
        h.extend_from_slice(&data_len.to_le_bytes());
        h
    }
}

/// Turns float samples into WAV bytes, keeping the dither state between blocks.
pub struct Encoder {
    pub format: WavFormat,
    rng: u32,
}

impl Encoder {
    pub fn new(format: WavFormat) -> Encoder { Encoder { format, rng: 0x1234_5678 } }

    fn noise(&mut self) -> f32 {
        // xorshift, uniform in -0.5..0.5
        self.rng ^= self.rng << 13;
        self.rng ^= self.rng >> 17;
        self.rng ^= self.rng << 5;
        self.rng as f32 / u32::MAX as f32 - 0.5
    }

    pub fn push(&mut self, s: f32, out: &mut Vec<u8>) {
        let s = if s.is_finite() { s } else { 0.0 };
        match self.format {
            WavFormat::Pcm16 => {
                // TPDF: two uniform noises of 1 LSB each, before rounding
                let d = self.noise() + self.noise();
                let v = (s.clamp(-1.0, 1.0) * 32767.0 + d).round().clamp(-32768.0, 32767.0) as i16;
                out.extend_from_slice(&v.to_le_bytes());
            }
            WavFormat::Pcm24 => {
                let v = (s.clamp(-1.0, 1.0) * 8_388_607.0).round() as i32;
                out.extend_from_slice(&v.to_le_bytes()[..3]);
            }
            WavFormat::Float32 => out.extend_from_slice(&s.to_le_bytes()),
        }
    }

    pub fn bytes_per_sample(&self) -> u32 { self.format.bytes() as u32 }
}

/// Write interleaved stereo f32 samples as a WAV file in `format`.
pub fn write_wav_as(path: &std::path::Path, samples: &[f32], sample_rate: u32, format: WavFormat) -> std::io::Result<()> {
    let mut enc = Encoder::new(format);
    let data_len = samples.len() as u32 * enc.bytes_per_sample();
    let mut b = format.header(sample_rate, data_len);
    b.reserve(data_len as usize);
    for s in samples { enc.push(*s, &mut b); }
    std::fs::write(path, b)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trois_formats_relus_par_le_decodeur() {
        let dir = std::env::temp_dir().join(format!("loom-wavout-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let sig: Vec<f32> = (0..2000).map(|i| (i as f32 * 0.05).sin() * 0.5).collect();
        for (f, tol) in [(WavFormat::Pcm16, 2.0 / 32768.0), (WavFormat::Pcm24, 1.0 / 4_000_000.0), (WavFormat::Float32, 0.0)] {
            let p = dir.join(format!("{}.wav", f.bytes()));
            write_wav_as(&p, &sig, 44_100, f).unwrap();
            let d = crate::sample::SampleData::from_file(&p).unwrap();
            assert_eq!(d.sample_rate, 44_100.0);
            assert_eq!(d.channels.len(), 2);
            for (i, x) in d.channels[0].iter().enumerate() {
                assert!((x - sig[i * 2]).abs() <= tol, "{f:?} échantillon {i}");
            }
        }
        // en flottant, une crête au-dessus de 0 dBFS reste intacte
        let p = dir.join("chaud.wav");
        write_wav_as(&p, &[1.5, -1.5], 48_000, WavFormat::Float32).unwrap();
        assert_eq!(crate::sample::SampleData::from_file(&p).unwrap().channels[0][0], 1.5);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn dither_decorrelle_le_silence_presque_nul() {
        // un signal très faible ne disparaît pas en 16 bits : le dither le garde en moyenne
        let mut enc = Encoder::new(WavFormat::Pcm16);
        let mut out = Vec::new();
        let level = 0.3 / 32767.0;
        for _ in 0..20_000 { enc.push(level, &mut out); }
        let mean: f64 = out.chunks_exact(2).map(|c| i16::from_le_bytes([c[0], c[1]]) as f64).sum::<f64>() / 20_000.0;
        assert!((mean - 0.3).abs() < 0.05, "moyenne {mean}");
        assert_eq!(WavFormat::from_name("--32f"), Some(WavFormat::Float32));
        assert_eq!(WavFormat::from_name("24"), Some(WavFormat::Pcm24));
    }
}
