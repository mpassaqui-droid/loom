//! Omarchy theme integration.
//!
//! Omarchy writes the active theme to
//! `~/.local/state/omarchy/current/theme/colors.toml` (flat `key = "#rrggbb"`
//! lines). LOOM reads it so the app matches the desktop, and re-reads it when
//! the file changes so `omarchy theme set …` recolours LOOM live. Off Omarchy,
//! LOOM falls back to its own retro green look.

use std::path::PathBuf;
use std::time::{Duration, Instant, SystemTime};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rgb(pub u8, pub u8, pub u8);

impl Rgb {
    pub fn parse(hex: &str) -> Option<Rgb> {
        let h = hex.trim().trim_start_matches('#');
        if h.len() != 6 { return None; }
        let c = |i: usize| u8::from_str_radix(&h[i..i + 2], 16).ok();
        Some(Rgb(c(0)?, c(2)?, c(4)?))
    }
    /// Blend towards `other` by `t` (0 = self, 1 = other).
    pub fn mix(self, other: Rgb, t: f32) -> Rgb {
        let m = |a: u8, b: u8| (a as f32 + (b as f32 - a as f32) * t.clamp(0.0, 1.0)).round() as u8;
        Rgb(m(self.0, other.0), m(self.1, other.1), m(self.2, other.2))
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Theme {
    pub name: String,
    pub dark: bool,
    pub background: Rgb,
    pub panel: Rgb,
    pub foreground: Rgb,
    pub dim: Rgb,
    pub accent: Rgb,
    pub selection: Rgb,
    /// Drum hits on the grid.
    pub drum: Rgb,
    /// Melodic notes on the grid.
    pub note: Rgb,
    pub error: Rgb,
}

impl Default for Theme {
    /// LOOM's own look: phosphor green on black, like the browser prototype.
    fn default() -> Theme {
        Theme {
            name: "loom".into(),
            dark: true,
            background: Rgb(0x0a, 0x0f, 0x0a),
            panel: Rgb(0x10, 0x18, 0x10),
            foreground: Rgb(0x9c, 0xff, 0x9c),
            dim: Rgb(0x3c, 0x6e, 0x3c),
            accent: Rgb(0x39, 0xff, 0x14),
            selection: Rgb(0x1f, 0x3a, 0x1f),
            drum: Rgb(0x39, 0xff, 0x14),
            note: Rgb(0x00, 0xe5, 0xff),
            error: Rgb(0xff, 0x55, 0x55),
        }
    }
}

impl Theme {
    /// Build a theme from the text of an Omarchy `colors.toml`.
    pub fn from_colors_toml(name: &str, text: &str) -> Theme {
        let mut map = std::collections::HashMap::new();
        for line in text.lines() {
            let Some((k, v)) = line.split_once('=') else { continue };
            map.insert(k.trim().to_string(), v.trim().trim_matches('"').to_string());
        }
        let d = Theme::default();
        let get = |k: &str| map.get(k).and_then(|v| Rgb::parse(v));
        let background = get("background").unwrap_or(d.background);
        let foreground = get("foreground").unwrap_or(d.foreground);
        let accent = get("accent").or_else(|| get("blue")).unwrap_or(d.accent);
        Theme {
            name: name.to_string(),
            dark: map.get("mode").is_none_or(|m| m != "light"),
            background,
            panel: get("lighter_background").unwrap_or_else(|| background.mix(foreground, 0.06)),
            foreground,
            dim: get("dark_foreground").or_else(|| get("light_foreground")).unwrap_or_else(|| background.mix(foreground, 0.4)),
            accent,
            selection: get("selection").unwrap_or_else(|| background.mix(accent, 0.25)),
            drum: accent,
            note: get("bright_green").or_else(|| get("green")).unwrap_or(d.note),
            error: get("red").unwrap_or(d.error),
        }
    }

    fn state_dir() -> Option<PathBuf> {
        let base = std::env::var_os("XDG_STATE_HOME").map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local/state")))?;
        Some(base.join("omarchy/current"))
    }

    /// Whether the GUI should draw larger text (accessibility). Persisted next
    /// to the theme so it survives restarts; default is `false` (normal size).
    pub fn ui_large() -> bool {
        Self::state_dir()
            .map(|d| d.join("loom-ui-large"))
            .is_some_and(|p| std::fs::read_to_string(&p).is_ok_and(|s| s.trim() == "1"))
    }

    /// Persist the GUI large-text accessibility flag (mirror of `ui_large`).
    pub fn set_ui_large(large: bool) {
        let Some(dir) = Self::state_dir() else { return };
        if large {
            let _ = std::fs::write(dir.join("loom-ui-large"), "1\n");
        } else {
            let _ = std::fs::remove_file(dir.join("loom-ui-large"));
        }
    }

    /// Whether the SCÈNE draws any text (title, section, bar, hints, scene name).
    /// Persisted next to the theme so a restart keeps the choice. The default is
    /// `false`: the stage is a pure-visual projection, and the text only comes
    /// back if the owner asks for it with `T`.
    pub fn stage_text() -> bool {
        Self::state_dir()
            .map(|d| d.join("loom-stage-text"))
            .is_some_and(|p| std::fs::read_to_string(&p).is_ok_and(|s| s.trim() == "1"))
    }

    /// Persist the SCÈNE text flag (mirror of `stage_text`).
    pub fn set_stage_text(show: bool) {
        let Some(dir) = Self::state_dir() else { return };
        if show {
            let _ = std::fs::write(dir.join("loom-stage-text"), "1\n");
        } else {
            let _ = std::fs::remove_file(dir.join("loom-stage-text"));
        }
    }

    /// Path of the active Omarchy `colors.toml`, if this is an Omarchy system.
    pub fn omarchy_colors_path() -> Option<PathBuf> {
        Self::state_dir().map(|d| d.join("theme/colors.toml")).filter(|p| p.exists())
    }

    /// One of the web themes (`web/index.html`), mapped onto the app's roles.
    pub fn web(name: &str) -> Option<Theme> {
        let (n, c) = crate::themes_web::WEB_THEMES.iter().find(|(n, _)| *n == name)?;
        let [bg, green, dim, glow, _amber, cyan] = c.map(|h| Rgb::parse(h).unwrap_or(Rgb(0, 0, 0)));
        let lum = 0.299 * bg.0 as f32 + 0.587 * bg.1 as f32 + 0.114 * bg.2 as f32;
        Some(Theme {
            name: n.to_string(),
            dark: lum < 128.0,
            background: bg,
            panel: bg.mix(green, 0.06),
            foreground: glow,
            dim,
            accent: green,
            selection: bg.mix(green, 0.2),
            drum: green,
            note: cyan,
            error: Theme::default().error,
        })
    }

    /// Names of the themes Alt+C cycles through: Omarchy if present, the web themes, then LOOM's own.
    pub fn names() -> Vec<String> {
        let mut v = Vec::new();
        if Self::omarchy_colors_path().is_some() { v.push("omarchy".to_string()); }
        v.extend(crate::themes_web::WEB_THEMES.iter().map(|(n, _)| n.to_string()));
        v.push("loom".to_string());
        v
    }

    fn choice_path() -> Option<PathBuf> {
        let base = std::env::var_os("XDG_STATE_HOME").map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local/state")))?;
        Some(base.join("loom/theme"))
    }

    /// Remember the chosen theme for the next launch.
    pub fn save_choice(name: &str) {
        let Some(p) = Self::choice_path() else { return };
        if let Some(d) = p.parent() { let _ = std::fs::create_dir_all(d); }
        let _ = std::fs::write(p, format!("{name}\n"));
    }

    /// A theme by name: "omarchy", "loom" or a web theme.
    pub fn named(name: &str) -> Theme {
        match name {
            "loom" => Theme::default(),
            "omarchy" => Self::omarchy().unwrap_or_default(),
            n => Self::web(n).unwrap_or_default(),
        }
    }

    /// The theme after `current` in `names()`.
    pub fn next_after(current: &str) -> Theme {
        let names = Self::names();
        let i = names.iter().position(|n| n == current).map_or(0, |i| (i + 1) % names.len());
        Self::named(&names[i])
    }

    /// The saved choice, else the active Omarchy theme, else LOOM's default look.
    pub fn load() -> Theme {
        let saved = Self::choice_path().and_then(|p| std::fs::read_to_string(p).ok()).map(|s| s.trim().to_string());
        match saved.as_deref() {
            Some("loom") => Theme::default(),
            Some(n) if n != "omarchy" && Self::web(n).is_some() => Self::web(n).unwrap_or_default(),
            _ => Self::omarchy().unwrap_or_default(),
        }
    }

    fn omarchy() -> Option<Theme> {
        let path = Self::omarchy_colors_path()?;
        let name = Self::state_dir()
            .and_then(|d| std::fs::read_to_string(d.join("theme.name")).ok())
            .map(|s| s.trim().to_string())
            .unwrap_or_else(|| "omarchy".into());
        std::fs::read_to_string(&path).ok().map(|text| Theme::from_colors_toml(&name, &text))
    }
}

/// Polls the Omarchy theme file (at most once a second) and hands back a new
/// `Theme` when it changed. Call `poll()` from the UI loop.
pub struct ThemeWatcher {
    last_check: Instant,
    stamp: Option<SystemTime>,
}

impl Default for ThemeWatcher {
    fn default() -> Self { ThemeWatcher { last_check: Instant::now(), stamp: Self::stamp() } }
}

impl ThemeWatcher {
    fn stamp() -> Option<SystemTime> {
        // `omarchy theme set` swaps the directory, so follow the symlink
        Theme::omarchy_colors_path().and_then(|p| std::fs::metadata(p).ok()).and_then(|m| m.modified().ok())
    }
    pub fn poll(&mut self) -> Option<Theme> {
        if self.last_check.elapsed() < Duration::from_secs(1) { return None; }
        self.last_check = Instant::now();
        let now = Self::stamp();
        if now != self.stamp { self.stamp = now; Some(Theme::load()) } else { None }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_omarchy_colors() {
        let t = Theme::from_colors_toml("matte-black", "mode = \"dark\"\naccent = \"#e68e0d\"\nbackground = \"#121212\"\nforeground = \"#bebebe\"\n");
        assert_eq!(t.accent, Rgb(0xe6, 0x8e, 0x0d));
        assert_eq!(t.background, Rgb(0x12, 0x12, 0x12));
        assert!(t.dark);
        assert_eq!(t.name, "matte-black");
    }

    #[test]
    fn light_mode_and_bad_values_fall_back() {
        let t = Theme::from_colors_toml("x", "mode = \"light\"\naccent = \"oops\"\n");
        assert!(!t.dark);
        assert_eq!(t.accent, Theme::default().accent);
        assert_eq!(Rgb(0, 0, 0).mix(Rgb(200, 100, 50), 0.5), Rgb(100, 50, 25));
    }

    #[test]
    fn themes_du_web() {
        assert!(crate::themes_web::WEB_THEMES.len() >= 60);
        let t = Theme::web("crt").unwrap();
        assert_eq!(t.background, Rgb(0x0b, 0x0f, 0x0b));
        assert_eq!(t.accent, Rgb(0x5c, 0xff, 0x4a));
        assert!(t.dark);
        assert!(Theme::web("absent").is_none());
        let n = Theme::names();
        assert_eq!(n.last().map(String::as_str), Some("loom"));
        // le cycle passe par tous les noms et revient au début
        assert_eq!(Theme::next_after("loom").name, Theme::named(&n[0]).name);
        assert_eq!(Theme::named("loom"), Theme::default());
    }
}
