//! "Exporter WAV": bounce the song to a file, without freezing the window.
//!
//! The rendering runs on its own thread; the app polls the returned channel
//! each frame and shows the result in the status bar.

use std::path::{Path, PathBuf};
use std::sync::mpsc::{channel, Receiver};

use eframe::egui;
use loom_app::Library;
use loom_core::Program;

/// Bars exported when the code has no `song:` arrangement.
pub const DEFAULT_BARS: u32 = 8;

/// How many bars to export: the whole `song:` if there is one, else 8 bars.
pub fn bars_to_export(program: &Program) -> u32 {
    match program.len_bars() {
        0 => DEFAULT_BARS,
        song_bars => song_bars,
    }
}

/// Where the WAV goes: next to the song, same name with `.wav`.
/// An untitled buffer exports into the user's song folder.
pub fn wav_path(song: Option<&Path>) -> PathBuf {
    match song {
        Some(path) => path.with_extension("wav"),
        None => Library::user_dir().join("sans-titre.wav"),
    }
}

/// The result of an export: the file written, or an error message in French.
pub type ExportResult = Result<PathBuf, String>;

/// Render `bars` bars of `src` and write them to `path` on a background thread.
/// The window is asked to repaint when it's done, so the message shows at once.
pub fn spawn(src: String, bars: u32, path: PathBuf, sample_rate: u32, ctx: egui::Context) -> Receiver<ExportResult> {
    let (tx, rx) = channel();
    std::thread::spawn(move || {
        let samples = loom_audio::render(&src, bars, sample_rate);
        let result = path
            .parent()
            .map_or(Ok(()), std::fs::create_dir_all)
            .and_then(|_| loom_audio::write_wav(&path, &samples, sample_rate))
            .map(|_| path.clone())
            .map_err(|e| format!("export impossible ({}) : {e}", path.display()));
        let _ = tx.send(result);
        ctx.request_repaint();
    });
    rx
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn eight_bars_without_a_song() {
        assert_eq!(bars_to_export(&Program::parse("kick: x . x .")), 8);
    }

    #[test]
    fn whole_song_when_arranged() {
        let p = Program::parse("song: intro*4 drop*8 outro*2\nkick: x . x .");
        assert_eq!(bars_to_export(&p), 14);
    }

    #[test]
    fn wav_goes_next_to_the_song() {
        assert_eq!(wav_path(Some(Path::new("/tmp/songs/techno.loom"))), PathBuf::from("/tmp/songs/techno.wav"));
        assert_eq!(wav_path(None).extension().unwrap(), "wav");
    }
}
