//! STEMS: every voice exported to its own WAV, to mix the song in a DAW.
//!
//! The files go into a folder next to the song: `techno.loom` →
//! `techno-stems/kick.wav`, `techno-stems/bass.wav`… Each voice is rendered
//! through the same master chain as the full song (like the browser), 8 bars
//! or the whole `song:`. It runs on a background thread; the app polls
//! [`StemsJob::poll`] each frame to show the progress.

use std::path::{Path, PathBuf};
use std::sync::mpsc::{channel, Receiver};

use crate::Library;
use loom_audio::Sequencer;
use loom_core::Program;

/// Bars rendered when the code has no `song:` arrangement.
pub const DEFAULT_BARS: u32 = 8;
/// Seconds of silence added at the end so reverb and echo can ring out.
const TAIL_SECONDS: f64 = 2.0;

/// How many bars to render: the whole `song:`, else 8.
pub fn bars_to_render(program: &Program) -> u32 {
    match program.len_bars() {
        0 => DEFAULT_BARS,
        bars => bars,
    }
}

/// The stems folder: `<song>-stems` next to the song, or
/// `~/Music/loom/sans-titre-stems` for a song not saved yet.
pub fn stems_dir(song: Option<&Path>) -> PathBuf {
    match song {
        Some(path) => {
            let stem = path.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_else(|| "loom".into());
            path.with_file_name(format!("{stem}-stems"))
        }
        None => Library::user_dir().join("sans-titre-stems"),
    }
}

/// A voice's file in the folder: `<dir>/<voice>.wav`. Characters that don't
/// belong in a file name become `_`.
pub fn stem_path(dir: &Path, voice: &str) -> PathBuf {
    let safe: String = voice.chars().map(|c| if c.is_alphanumeric() || c == '-' || c == '_' { c } else { '_' }).collect();
    dir.join(format!("{}.wav", if safe.is_empty() { "voix".into() } else { safe }))
}

/// The voice names, once each, in the code's order.
pub fn voice_names(program: &Program) -> Vec<String> {
    let mut names: Vec<String> = Vec::new();
    for v in &program.voices {
        if !names.contains(&v.name) {
            names.push(v.name.clone());
        }
    }
    names
}

/// News from the stems thread.
enum Message {
    /// Starting voice number `index` (0-based).
    Started { index: usize, voice: String },
    /// All done: the folder and how many files, or an error in French.
    Finished(Result<(PathBuf, usize), String>),
}

/// A stems export running in the background.
pub struct StemsJob {
    rx: Receiver<Message>,
    pub total: usize,
    /// Voices finished so far, and the one being rendered.
    pub done: usize,
    pub current: String,
    pub dir: PathBuf,
}

impl StemsJob {
    /// Start rendering `src`. `wake` is called after each step (the GUI asks
    /// for a repaint with it). `None` if the code has no voices.
    pub fn start(src: String, song: Option<&Path>, sample_rate: u32, wake: impl Fn() + Send + 'static) -> Option<StemsJob> {
        let program = Program::parse(&src);
        let names = voice_names(&program);
        if names.is_empty() {
            return None;
        }
        let dir = stems_dir(song);
        let bars = bars_to_render(&program);
        let (tx, rx) = channel();
        let job = StemsJob { rx, total: names.len(), done: 0, current: String::new(), dir: dir.clone() };
        std::thread::spawn(move || {
            let result = std::fs::create_dir_all(&dir).map_err(|e| format!("STEMS : impossible de créer {} : {e}", dir.display()));
            let result = result.and_then(|_| {
                for (index, voice) in names.iter().enumerate() {
                    let _ = tx.send(Message::Started { index, voice: voice.clone() });
                    wake();
                    let audio = render_stem(&program, bars, sample_rate, voice);
                    let path = stem_path(&dir, voice);
                    loom_audio::write_wav(&path, &audio, sample_rate).map_err(|e| format!("STEMS : {} : {e}", path.display()))?;
                }
                Ok((dir.clone(), names.len()))
            });
            let _ = tx.send(Message::Finished(result));
            wake();
        });
        Some(job)
    }

    /// Read the thread's news. Returns the result once it's finished.
    pub fn poll(&mut self) -> Option<Result<(PathBuf, usize), String>> {
        loop {
            match self.rx.try_recv() {
                Ok(Message::Started { index, voice }) => {
                    self.done = index;
                    self.current = voice;
                }
                Ok(Message::Finished(result)) => return Some(result),
                Err(std::sync::mpsc::TryRecvError::Empty) => return None,
                Err(std::sync::mpsc::TryRecvError::Disconnected) => return Some(Err("STEMS : l'export s'est interrompu.".into())),
            }
        }
    }

    /// "STEMS 2/5 · bass"
    pub fn progress_text(&self) -> String {
        format!("STEMS {}/{} · {}", self.done + 1, self.total, self.current)
    }

    /// 0..1, for a progress bar.
    pub fn fraction(&self) -> f32 {
        self.done as f32 / self.total.max(1) as f32
    }
}

/// One voice alone, through the master chain: the same steps as
/// `loom_audio::render_stems`, one voice at a time so we can show progress.
fn render_stem(program: &Program, bars: u32, sample_rate: u32, voice: &str) -> Vec<f32> {
    let len = program.len_bars() as i64;
    let seconds: f64 = (0..bars as i64).map(|b| program.bar_seconds(if len > 0 { b.rem_euclid(len) } else { b })).sum();
    let frames = ((seconds + TAIL_SECONDS) * sample_rate as f64) as usize;
    let mut seq = Sequencer::new(sample_rate as f32, loom_audio::default_bank());
    seq.set_stem(Some(voice));
    seq.set_program(program.clone());
    seq.play();
    let mut out = vec![0.0; frames * 2];
    let body = ((seconds * sample_rate as f64) as usize * 2).min(out.len());
    seq.render(&mut out[..body], 2);
    seq.stop();
    seq.render(&mut out[body..], 2);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stems_folder_sits_next_to_the_song() {
        assert_eq!(stems_dir(Some(Path::new("/m/techno.loom"))), PathBuf::from("/m/techno-stems"));
        assert!(stems_dir(None).ends_with("sans-titre-stems"));
        assert_eq!(stem_path(Path::new("/m/techno-stems"), "bass"), PathBuf::from("/m/techno-stems/bass.wav"));
        assert_eq!(stem_path(Path::new("/d"), "a/b c"), PathBuf::from("/d/a_b_c.wav"));
    }

    #[test]
    fn eight_bars_or_the_whole_song() {
        assert_eq!(bars_to_render(&Program::parse("kick: x")), 8);
        assert_eq!(bars_to_render(&Program::parse("song: a*4 b*2\nkick: x")), 6);
    }

    #[test]
    fn one_stem_matches_the_engine_render() {
        let src = "bpm: 240\nkick: x . x .\nlead: c4 . e4 .";
        let program = Program::parse(src);
        let ours = render_stem(&program, 1, 8000, "lead");
        let engine = loom_audio::render_stems(src, 1, 8000);
        assert_eq!(voice_names(&program), ["kick", "lead"]);
        assert_eq!(engine[1].0, "lead");
        assert_eq!(ours, engine[1].1);
    }

    #[test]
    fn job_writes_one_wav_per_voice() {
        let dir = std::env::temp_dir().join(format!("loom-stems-test-{}", std::process::id()));
        let song = dir.join("essai.loom");
        let mut job = StemsJob::start("bpm: 240\nkick: x . x .\nhat: x x x x".into(), Some(&song), 8000, || {}).unwrap();
        let result = loop {
            if let Some(r) = job.poll() { break r; }
            std::thread::sleep(std::time::Duration::from_millis(5));
        };
        let (folder, count) = result.unwrap();
        assert_eq!((folder.clone(), count), (dir.join("essai-stems"), 2));
        assert!(folder.join("kick.wav").is_file() && folder.join("hat.wav").is_file());
        let _ = std::fs::remove_dir_all(&dir);
        assert!(StemsJob::start("# rien".into(), None, 8000, || {}).is_none());
    }
}
