//! The song library: the `.loom` files LOOM can open.
//!
//! Songs are looked for in the user's own folder (`~/Music/loom`, created on
//! demand), the current directory, and the bundled examples installed next to
//! the app (`~/.local/share/loom/examples`).

use std::path::{Path, PathBuf};

pub struct Library;

impl Library {
    /// Where new songs are saved by default.
    pub fn user_dir() -> PathBuf {
        let home = std::env::var_os("HOME").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("."));
        let music = std::env::var_os("XDG_MUSIC_DIR").map(PathBuf::from).unwrap_or_else(|| home.join("Music"));
        music.join("loom")
    }

    /// Bundled examples installed by `install.sh`.
    pub fn examples_dir() -> PathBuf {
        let data = std::env::var_os("XDG_DATA_HOME").map(PathBuf::from).unwrap_or_else(|| {
            std::env::var_os("HOME").map(PathBuf::from).unwrap_or_default().join(".local/share")
        });
        data.join("loom/examples")
    }

    /// All `.loom` files, grouped by folder label, each list sorted by name.
    pub fn scan() -> Vec<(String, Vec<PathBuf>)> {
        let mut groups = Vec::new();
        let mut seen: Vec<PathBuf> = Vec::new();
        let cwd = std::env::current_dir().unwrap_or_default();
        for (label, dir) in [("Mes morceaux", Self::user_dir()), ("Dossier courant", cwd), ("Exemples", Self::examples_dir())] {
            let Ok(canon) = dir.canonicalize() else { continue };
            if seen.contains(&canon) { continue; }
            seen.push(canon);
            let files = Self::list(&dir);
            if !files.is_empty() { groups.push((label.to_string(), files)); }
        }
        groups
    }

    pub fn list(dir: &Path) -> Vec<PathBuf> {
        let mut files: Vec<PathBuf> = std::fs::read_dir(dir)
            .into_iter()
            .flatten()
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.extension().is_some_and(|e| e == "loom"))
            .collect();
        files.sort();
        files
    }

    /// A free file name like `~/Music/loom/sans-titre-3.loom`.
    pub fn new_song_path() -> PathBuf {
        let dir = Self::user_dir();
        (1..).map(|i| dir.join(if i == 1 { "sans-titre.loom".to_string() } else { format!("sans-titre-{i}.loom") }))
            .find(|p| !p.exists())
            .unwrap()
    }

    fn last_file_record() -> PathBuf {
        let state = std::env::var_os("XDG_STATE_HOME").map(PathBuf::from).unwrap_or_else(|| {
            std::env::var_os("HOME").map(PathBuf::from).unwrap_or_default().join(".local/state")
        });
        state.join("loom/last")
    }

    /// The song opened last time (shared by the TUI and the GUI), if it still exists.
    pub fn last_file() -> Option<PathBuf> {
        let text = std::fs::read_to_string(Self::last_file_record()).ok()?;
        Some(PathBuf::from(text.trim())).filter(|p| p.is_file())
    }

    /// Remember `path` as the song to reopen next time.
    pub fn remember(path: &Path) {
        let record = Self::last_file_record();
        let abs = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
        if let Some(parent) = record.parent() { let _ = std::fs::create_dir_all(parent); }
        let _ = std::fs::write(record, abs.to_string_lossy().as_bytes());
    }

    /// A free file name for a live recording: `~/Music/loom/enregistrements/<song>-rec-N.wav`.
    pub fn new_recording_path(song: Option<&Path>) -> PathBuf {
        let dir = Self::user_dir().join("enregistrements");
        let stem = song.and_then(|p| p.file_stem()).map(|s| s.to_string_lossy().into_owned()).unwrap_or_else(|| "loom".into());
        (1..).map(|i| dir.join(format!("{stem}-rec-{i}.wav"))).find(|p| !p.exists()).unwrap()
    }

    /// Save, creating the parent folder if needed.
    pub fn save(path: &Path, src: &str) -> std::io::Result<()> {
        if let Some(parent) = path.parent() { std::fs::create_dir_all(parent)?; }
        std::fs::write(path, src)
    }
}
