//! LOOM native sound.
//!
//! - [`synth`]: the instruments (drums + oscillators), ported from the browser engine.
//! - [`seq`]: the sample-clock sequencer that plays a `loom_core::Program`.
//! - [`Engine`] (feature `device`): live output on the system sound card.
//! - [`render`] / [`write_wav`]: offline bounce to a WAV file.

pub mod seq;
pub mod synth;
#[cfg(feature = "device")]
mod device;

#[cfg(feature = "device")]
pub use device::Engine;
pub use seq::{Sequencer, Status};

use loom_core::Program;

/// Render `bars` bars of LOOM source offline. Returns interleaved stereo f32.
pub fn render(src: &str, bars: u32, sample_rate: u32) -> Vec<f32> {
    let program = Program::parse(src);
    let seconds: f64 = (0..bars as i64).map(|b| {
        let len = program.len_bars() as i64;
        program.bar_seconds(if len > 0 { b.rem_euclid(len) } else { b })
    }).sum();
    let tail = 0.5; // let the last notes ring out
    let frames = ((seconds + tail) * sample_rate as f64) as usize;
    let mut seq = Sequencer::new(sample_rate as f32);
    seq.set_program(program);
    seq.play();
    let mut out = vec![0.0; frames * 2];
    let body = (seconds * sample_rate as f64) as usize * 2;
    seq.render(&mut out[..body], 2);
    seq.stop();
    seq.render(&mut out[body..], 2);
    out
}

/// Write interleaved stereo f32 samples as a 16-bit PCM WAV file.
pub fn write_wav(path: &std::path::Path, samples: &[f32], sample_rate: u32) -> std::io::Result<()> {
    let data_len = (samples.len() * 2) as u32;
    let mut b = Vec::with_capacity(44 + data_len as usize);
    b.extend_from_slice(b"RIFF");
    b.extend_from_slice(&(36 + data_len).to_le_bytes());
    b.extend_from_slice(b"WAVEfmt ");
    b.extend_from_slice(&16u32.to_le_bytes());
    b.extend_from_slice(&1u16.to_le_bytes()); // PCM
    b.extend_from_slice(&2u16.to_le_bytes()); // stereo
    b.extend_from_slice(&sample_rate.to_le_bytes());
    b.extend_from_slice(&(sample_rate * 4).to_le_bytes()); // byte rate
    b.extend_from_slice(&4u16.to_le_bytes()); // block align
    b.extend_from_slice(&16u16.to_le_bytes()); // bits
    b.extend_from_slice(b"data");
    b.extend_from_slice(&data_len.to_le_bytes());
    for s in samples {
        b.extend_from_slice(&((s.clamp(-1.0, 1.0) * 32767.0) as i16).to_le_bytes());
    }
    std::fs::write(path, b)
}

#[cfg(test)]
mod tests {
    #[test]
    fn every_example_song_renders_cleanly() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
        let mut n = 0;
        for entry in std::fs::read_dir(&root).unwrap() {
            let path = entry.unwrap().path();
            if path.extension().is_some_and(|e| e == "loom") {
                let src = std::fs::read_to_string(&path).unwrap();
                let out = super::render(&src, 2, 22050);
                assert!(out.iter().all(|x| x.is_finite() && x.abs() <= 1.0), "{}", path.display());
                n += 1;
            }
        }
        assert!(n > 10, "expected the example .loom files, found {n}");
    }
}
