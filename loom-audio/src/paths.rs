//! Où LOOM range ses affaires, sur chaque système.
//!
//! - macOS et Linux : comme avant (XDG) : `~/.local/share/loom` (mes sons,
//!   exemples), `~/.local/state/loom` (dernier morceau, thème, MIDI learn),
//!   `~/Music/loom` (mes morceaux). Rien ne bouge pour qui a déjà LOOM.
//! - Windows : `%APPDATA%\loom`, `%LOCALAPPDATA%\loom`, `%USERPROFILE%\Music\loom`
//!   (Windows n'a pas de `HOME`).
//!
//! Les variables `XDG_*` gardent la priorité partout où elles sont définies.

use std::ffi::OsString;
use std::path::PathBuf;

fn var(name: &str) -> Option<PathBuf> { std::env::var_os(name).filter(|v| !v.is_empty()).map(PathBuf::from) }

/// Le dossier personnel : `HOME`, sinon `USERPROFILE` (Windows).
pub fn home() -> Option<PathBuf> { var("HOME").or_else(|| var("USERPROFILE")) }

/// Les données : mes sons, les exemples.
pub fn data_dir() -> Option<PathBuf> {
    resolve(var("XDG_DATA_HOME"), var("APPDATA"), home(), ".local/share")
}

/// L'état : dernier morceau, thème, réglages appris.
pub fn state_dir() -> Option<PathBuf> {
    resolve(var("XDG_STATE_HOME"), var("LOCALAPPDATA"), home(), ".local/state")
}

/// Mes morceaux.
pub fn songs_dir() -> PathBuf {
    var("XDG_MUSIC_DIR").or_else(|| home().map(|h| h.join("Music"))).unwrap_or_else(|| PathBuf::from(".")).join("loom")
}

/// `xdg` s'il est défini ; sur Windows le dossier système `win` ; sinon `home/fallback` ; puis `loom`.
fn resolve(xdg: Option<PathBuf>, win: Option<PathBuf>, home: Option<PathBuf>, fallback: &str) -> Option<PathBuf> {
    let base = xdg.or(if cfg!(windows) { win } else { None }).or_else(|| home.map(|h| h.join(fallback)))?;
    Some(base.join("loom"))
}

/// Pour les tests : la même règle avec des valeurs données.
#[doc(hidden)]
pub fn resolve_with(xdg: Option<OsString>, win: Option<OsString>, home: Option<OsString>, fallback: &str) -> Option<PathBuf> {
    resolve(xdg.map(PathBuf::from), win.map(PathBuf::from), home.map(PathBuf::from), fallback)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn regles() {
        let p = |x: Option<&str>, w: Option<&str>, h: Option<&str>| resolve_with(x.map(Into::into), w.map(Into::into), h.map(Into::into), ".local/share");
        assert_eq!(p(Some("/x"), None, Some("/h")), Some(PathBuf::from("/x/loom")));
        assert_eq!(p(None, None, Some("/h")), Some(PathBuf::from("/h/.local/share/loom")));
        assert_eq!(p(None, None, None), None);
        // APPDATA ne compte que sur Windows
        let w = p(None, Some("C:/AppData"), Some("/h"));
        if cfg!(windows) { assert_eq!(w, Some(PathBuf::from("C:/AppData/loom"))); } else { assert_eq!(w, Some(PathBuf::from("/h/.local/share/loom"))); }
    }
}
