//! LOOM native sound.
//!
//! - [`seq`]: the sample-clock sequencer that plays a `loom_core::Program`.
//! - [`note`]: the contract between the sequencer and the sound modules.
//! - [`drums`], [`tone`], [`sample`]: the sound sources.
//! - [`inserts`]: the per-note effect chain; [`master`]: buses, sends, mastering.
//! - [`dsp`], [`clock`]: shared building blocks and musical time.
//! - [`Engine`] (feature `device`): live output on the system sound card.
//! - [`render`] / [`write_wav`]: offline bounce to a WAV file.

pub mod bake;
pub mod bytebeat;
pub mod clock;
pub mod drums;
pub mod dsp;
pub mod inserts;
pub mod master;
pub mod note;
pub mod sample;
pub mod seq;
pub mod tone;
pub mod truepeak;
pub mod wavout;
#[cfg(feature = "device")]
mod device;
#[cfg(feature = "device")]
pub mod midi;
#[cfg(feature = "device")]
pub mod input;

#[cfg(feature = "device")]
pub use device::Engine;
pub use wavout::{write_wav_as, WavFormat};
pub use sample::{load_user_sample, merged_bank, reload_bank, session_sample_count, SampleBank};
pub use seq::{Channel, Hit, MidiEvent, MidiNote, Sequencer, Status, SCOPE_LEN};

use loom_core::Program;
use std::sync::Arc;

/// The default sample bank for offline rendering: the disk samples plus the
/// WAVs loaded this session (see [`load_user_sample`]), so anything the user
/// picks in the SAMPLER panel is heard on the very next play.
pub fn default_bank() -> Arc<SampleBank> {
    // every call rebuilds from the session store (disk + user WAVs loaded in the
    // SAMPLER panel), so a file loaded at runtime is heard on the very next play.
    Arc::new(sample::merged_bank())
}

/// Render `bars` bars of LOOM source offline. Returns interleaved stereo f32.
pub fn render(src: &str, bars: u32, sample_rate: u32) -> Vec<f32> {
    render_voice(src, bars, sample_rate, None)
}

/// Stems: every voice rendered on its own (through the same master chain),
/// as `(voice name, interleaved stereo)` — the browser's STEMS button.
pub fn render_stems(src: &str, bars: u32, sample_rate: u32) -> Vec<(String, Vec<f32>)> {
    let mut names: Vec<String> = Vec::new();
    for v in Program::parse(src).voices {
        if !names.contains(&v.name) { names.push(v.name); }
    }
    names.into_iter().map(|n| { let audio = render_voice(src, bars, sample_rate, Some(&n)); (n, audio) }).collect()
}

fn render_voice(src: &str, bars: u32, sample_rate: u32, stem: Option<&str>) -> Vec<f32> {
    let program = Program::parse(src);
    let seconds: f64 = (0..bars as i64).map(|b| {
        let len = program.len_bars() as i64;
        program.bar_seconds(if len > 0 { b.rem_euclid(len) } else { b })
    }).sum();
    let tail = 2.0; // let reverb and delay tails ring out
    let frames = ((seconds + tail) * sample_rate as f64) as usize;
    let mut seq = Sequencer::new(sample_rate as f32, default_bank());
    seq.set_stem(stem);
    seq.set_program(program);
    seq.play();
    let mut out = vec![0.0; frames * 2];
    let body = (seconds * sample_rate as f64) as usize * 2;
    seq.render(&mut out[..body], 2);
    seq.stop();
    seq.render(&mut out[body..], 2);
    out
}

/// Write interleaved stereo f32 samples as a WAV file in the current export
/// format ([`WavFormat::current`]: 24 bits unless chosen otherwise).
pub fn write_wav(path: &std::path::Path, samples: &[f32], sample_rate: u32) -> std::io::Result<()> {
    wavout::write_wav_as(path, samples, sample_rate, WavFormat::current())
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
