//! LIEN / EXPORT / IMPORT: share a song with its knob settings.
//!
//! The codes themselves (`LOOM1:…`, browser links) are made and read by
//! `loom_app::share`; this module only moves the knob values in and out.

use std::path::{Path, PathBuf};

use loom_app::share::{self, ShareState};
use loom_app::Library;

use super::knobs::{Knob, Knobs};
use super::songline;

/// A `LOOM1:` code for this song and these knobs (the browser can import it).
pub fn export(code: &str, knobs: &Knobs) -> String {
    let value = |knob: Knob| Some(knobs.get(knob) as f64);
    share::share_code(&ShareState {
        code: code.to_string(),
        bpm: Some(loom_core::Program::parse(code).bpm),
        swing: value(Knob::Swing),
        echo: value(Knob::Echo),
        space: value(Knob::Space),
        volume: value(Knob::Volume),
        eq_low: value(Knob::EqLow),
        eq_mid: value(Knob::EqMid),
        eq_high: value(Knob::EqHigh),
    })
}

/// Where the terminal app writes a share code: `<song>.share.txt` next to the
/// song (`~/Music/loom/sans-titre.share.txt` for a song not saved yet).
#[allow(dead_code)] // used by the terminal app; the window copies to the clipboard
pub fn share_path(song: Option<&Path>) -> PathBuf {
    match song {
        Some(path) => path.with_extension("share.txt"),
        None => Library::user_dir().join("sans-titre.share.txt"),
    }
}

/// A song read from whatever was pasted: the code to put in the editor, and
/// the knobs (the current ones, changed by any value the code carried).
#[derive(Debug, PartialEq)]
pub struct Imported {
    pub code: String,
    pub knobs: Knobs,
    /// True when the paste carried knob values (a `LOOM1:` code).
    pub has_knobs: bool,
}

/// Read a `LOOM1:` code, a browser link or plain LOOM code.
/// `None` if it isn't any of these.
pub fn import(pasted: &str, current: &Knobs) -> Option<Imported> {
    let state = share::decode(pasted)?;
    let mut knobs = current.clone();
    let values = [
        (Knob::Swing, state.swing),
        (Knob::Echo, state.echo),
        (Knob::Space, state.space),
        (Knob::Volume, state.volume),
        (Knob::EqLow, state.eq_low),
        (Knob::EqMid, state.eq_mid),
        (Knob::EqHigh, state.eq_high),
    ];
    let mut has_knobs = false;
    for (knob, value) in values {
        if let Some(v) = value {
            knobs.set(knob, v as f32);
            has_knobs = true;
        }
    }
    // the browser's BPM field: only needed when the code has no tempo of its own
    let mut code = state.code;
    let has_tempo_line = code.lines().any(|l| {
        let head = l.split(':').next().unwrap_or("").trim().to_lowercase();
        l.contains(':') && (head == "bpm" || head == "tempo")
    });
    if let Some(bpm) = state.bpm.filter(|b| !has_tempo_line && (40.0..=300.0).contains(b) && *b != 120.0) {
        code = songline::set_bpm(&code, bpm.round() as u32);
    }
    Some(Imported { code, knobs, has_knobs })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn export_then_import_gives_back_song_and_knobs() {
        let mut knobs = Knobs::default();
        knobs.set(Knob::Space, 55.0);
        knobs.set(Knob::EqMid, -1.5);
        let code = "bpm: 96\nkick: x . x .\n";
        let text = export(code, &knobs);
        assert!(text.starts_with("LOOM1:"));
        let back = import(&text, &Knobs::default()).unwrap();
        assert_eq!(back, Imported { code: code.into(), knobs, has_knobs: true });
    }

    #[test]
    fn links_and_plain_code_keep_the_current_knobs() {
        let mut knobs = Knobs::default();
        knobs.set(Knob::Echo, 40.0);
        let link = format!("https://exemple.org/loom/index.html{}", share::link_fragment("hat: x x"));
        let got = import(&link, &knobs).unwrap();
        assert_eq!((got.code.as_str(), got.knobs.echo, got.has_knobs), ("hat: x x", 40.0, false));
        assert_eq!(import("kick: x . x .", &knobs).unwrap().code, "kick: x . x .");
        assert!(import("bonjour", &knobs).is_none());
    }

    #[test]
    fn share_file_sits_next_to_the_song() {
        assert_eq!(share_path(Some(Path::new("/m/techno.loom"))), PathBuf::from("/m/techno.share.txt"));
        assert!(share_path(None).ends_with("sans-titre.share.txt"));
    }

    #[test]
    fn browser_bpm_field_becomes_a_bpm_line_when_missing() {
        let state = ShareState { code: "kick: x . x .".into(), bpm: Some(132.0), ..Default::default() };
        let got = import(&share::share_code(&state), &Knobs::default()).unwrap();
        assert_eq!(got.code, "bpm: 132\nkick: x . x .");
    }
}
