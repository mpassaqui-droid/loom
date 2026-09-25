//! DÉMO: an automatic tour. LOOM plays each example song in turn on the
//! SCÈNE, changing song every 22 seconds (the browser's timing), until any
//! key or click stops it and brings back the user's own song.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

/// The browser's tour, in its order (`concrete` is `h_concrete.loom`, etc.).
pub const TOUR: [&str; 42] = [
    "afx", "gu", "mixtape", "monologue", "taqsim", "gamelan", "raga", "claviers", "basses", "design", "autechre",
    "expose", "evolution", "spaces", "newmusic", "odyssey", "frontier", "composer", "additive", "evolve", "harmony",
    "pulse", "concrete", "cologne", "buchla", "berlin", "spectral", "xenakis", "drone", "kraftwerk", "detroit", "dub",
    "eno", "jungle", "trap", "dubstep", "gabber", "afrobeat", "electro", "trance", "hyperpop", "gqom",
];

/// How long each song plays.
pub const SONG_TIME: Duration = Duration::from_secs(22);

/// The songs of the tour found in the library (`Library::scan()` groups):
/// the tour's names first, in order, looked for in the examples, then the
/// current folder, then the user's songs.
pub fn playlist(library: &[(String, Vec<PathBuf>)]) -> Vec<PathBuf> {
    let rank = |label: &str| match label {
        "Exemples" => 0,
        "Dossier courant" => 1,
        _ => 2,
    };
    let mut groups: Vec<&(String, Vec<PathBuf>)> = library.iter().collect();
    groups.sort_by_key(|(label, _)| rank(label));
    let stem = |p: &Path| p.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
    TOUR.iter()
        .filter_map(|name| {
            groups.iter().flat_map(|(_, files)| files).find(|f| {
                let s = stem(f);
                s == *name || s == format!("h_{name}")
            })
        })
        .cloned()
        .collect()
}

/// The tour in progress.
pub struct Demo {
    pub playlist: Vec<PathBuf>,
    /// The song playing now (index in `playlist`).
    pub index: usize,
    started: Instant,
}

impl Demo {
    /// Start with the first song. `None` if no example song was found.
    pub fn start(playlist: Vec<PathBuf>) -> Option<Demo> {
        (!playlist.is_empty()).then(|| Demo { playlist, index: 0, started: Instant::now() })
    }

    pub fn current(&self) -> &Path {
        &self.playlist[self.index]
    }

    /// Once the song has played long enough: move on and return the next one.
    pub fn next_if_due(&mut self) -> Option<&Path> {
        if self.started.elapsed() < SONG_TIME {
            return None;
        }
        self.index = (self.index + 1) % self.playlist.len();
        self.started = Instant::now();
        Some(self.current())
    }

    /// "DÉMO 3/42 · raga"
    pub fn text(&self) -> String {
        let name = self.current().file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
        format!("DÉMO {}/{} · {name}", self.index + 1, self.playlist.len())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn playlist_follows_the_tour_and_prefers_examples() {
        let library = vec![
            ("Dossier courant".to_string(), vec![PathBuf::from("/cwd/gu.loom"), PathBuf::from("/cwd/h_concrete.loom"), PathBuf::from("/cwd/mine.loom")]),
            ("Exemples".to_string(), vec![PathBuf::from("/ex/afx.loom"), PathBuf::from("/ex/gu.loom")]),
        ];
        assert_eq!(playlist(&library), [PathBuf::from("/ex/afx.loom"), PathBuf::from("/ex/gu.loom"), PathBuf::from("/cwd/h_concrete.loom")]);
        assert!(Demo::start(playlist(&[])).is_none());
        let demo = Demo::start(playlist(&library)).unwrap();
        assert_eq!(demo.text(), "DÉMO 1/3 · afx");
    }
}
