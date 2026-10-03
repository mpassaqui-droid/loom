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

    /// Every song of the picker, in order (Mes morceaux, dossier courant, exemples).
    pub fn all() -> Vec<PathBuf> {
        Self::scan().into_iter().flat_map(|(_, f)| f).collect()
    }

    /// The song `step` places after `current` in `list` (wraps around); the first one
    /// when `current` isn't in the list. Like the web's ▶ preset button.
    pub fn step_in(list: &[PathBuf], current: Option<&Path>, step: isize) -> Option<PathBuf> {
        if list.is_empty() { return None; }
        let n = list.len() as isize;
        let i = current.and_then(|c| list.iter().position(|p| p == c));
        let k = match i { Some(i) => (i as isize + step).rem_euclid(n), None => 0 };
        list.get(k as usize).cloned()
    }

    /// A song at random from `list`, never `current` when there is another one.
    pub fn random_in(list: &[PathBuf], current: Option<&Path>, seed: u64) -> Option<PathBuf> {
        let autres: Vec<&PathBuf> = list.iter().filter(|p| Some(p.as_path()) != current).collect();
        let pool: Vec<&PathBuf> = if autres.is_empty() { list.iter().collect() } else { autres };
        let x = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407) >> 33;
        pool.get((x % pool.len().max(1) as u64) as usize).map(|p| (*p).clone())
    }

    /// The trash folder of this system (macOS ~/.Trash, else the freedesktop one).
    pub fn trash_dir() -> PathBuf {
        let home = std::env::var_os("HOME").map(PathBuf::from).unwrap_or_default();
        if cfg!(target_os = "macos") { home.join(".Trash") } else {
            std::env::var_os("XDG_DATA_HOME").map(PathBuf::from).unwrap_or_else(|| home.join(".local/share")).join("Trash/files")
        }
    }

    /// Move one of the user's own songs to the trash (never an example, never outside
    /// `user_dir`). Returns where it went.
    pub fn trash_in(path: &Path, user_dir: &Path, trash: &Path) -> Result<PathBuf, String> {
        let canon = path.canonicalize().map_err(|e| e.to_string())?;
        let mine = user_dir.canonicalize().map_err(|e| e.to_string())?;
        if canon.parent() != Some(mine.as_path()) || canon.extension().is_none_or(|e| e != "loom") {
            return Err("seuls tes morceaux (Mes morceaux) peuvent aller à la corbeille".into());
        }
        std::fs::create_dir_all(trash).map_err(|e| e.to_string())?;
        let stem = canon.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
        let dest = (0..).map(|i| trash.join(if i == 0 { format!("{stem}.loom") } else { format!("{stem}-{i}.loom") }))
            .find(|p| !p.exists()).unwrap();
        std::fs::rename(&canon, &dest).map_err(|e| e.to_string())?;
        Ok(dest)
    }

    pub fn trash(path: &Path) -> Result<PathBuf, String> {
        Self::trash_in(path, &Self::user_dir(), &Self::trash_dir())
    }

    /// Save, creating the parent folder if needed.
    pub fn save(path: &Path, src: &str) -> std::io::Result<()> {
        if let Some(parent) = path.parent() { std::fs::create_dir_all(parent)?; }
        std::fs::write(path, src)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn suivant_precedent_hasard() {
        let l: Vec<PathBuf> = ["a.loom", "b.loom", "c.loom"].iter().map(PathBuf::from).collect();
        assert_eq!(Library::step_in(&l, Some(Path::new("c.loom")), 1), Some(l[0].clone()));
        assert_eq!(Library::step_in(&l, Some(Path::new("a.loom")), -1), Some(l[2].clone()));
        assert_eq!(Library::step_in(&l, None, 1), Some(l[0].clone()));
        assert_eq!(Library::step_in(&[], None, 1), None);
        for s in 0..50 { assert_ne!(Library::random_in(&l, Some(Path::new("b.loom")), s), Some(l[1].clone())); }
        assert_eq!(Library::random_in(&l[..1], Some(Path::new("a.loom")), 3), Some(l[0].clone()));
    }

    #[test]
    fn corbeille_seulement_mes_morceaux() {
        let base = std::env::temp_dir().join(format!("loom-corbeille-{}", std::process::id()));
        let (moi, ex, poub) = (base.join("moi"), base.join("ex"), base.join("poubelle"));
        for d in [&moi, &ex] { std::fs::create_dir_all(d).unwrap(); }
        std::fs::write(moi.join("m.loom"), "kick: x").unwrap();
        std::fs::write(moi.join("notes.txt"), "x").unwrap();
        std::fs::write(ex.join("e.loom"), "kick: x").unwrap();
        assert!(Library::trash_in(&ex.join("e.loom"), &moi, &poub).is_err(), "un exemple ne part pas");
        assert!(Library::trash_in(&moi.join("notes.txt"), &moi, &poub).is_err());
        let d = Library::trash_in(&moi.join("m.loom"), &moi, &poub).unwrap();
        assert!(d.exists() && !moi.join("m.loom").exists());
        std::fs::write(moi.join("m.loom"), "kick: x").unwrap();
        assert!(Library::trash_in(&moi.join("m.loom"), &moi, &poub).unwrap().ends_with("m-1.loom"), "pas d'écrasement");
        let _ = std::fs::remove_dir_all(&base);
    }
}
