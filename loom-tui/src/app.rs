//! App state and keyboard handling for the terminal client.

use crate::editor::Editor;
use loom_app::{Library, Theme, ThemeWatcher};
use loom_audio::Engine;
use loom_core::Program;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use std::path::PathBuf;
use std::time::Instant;

pub enum Mode {
    Edit,
    Help,
    /// The song picker: flattened `(group label, file)` entries and the selection.
    Browser { items: Vec<(String, PathBuf)>, selected: usize },
    /// Unsaved changes would be lost: what to do if the user confirms.
    Confirm { question: String, then: Pending },
}

#[derive(Clone)]
pub enum Pending { Quit, Open(PathBuf), New }

pub struct App {
    pub editor: Editor,
    pub path: Option<PathBuf>,
    pub engine: Result<Engine, String>,
    /// The code that is actually playing (last RUN), which drives the grid.
    pub program: Program,
    pub running_text: Option<String>,
    pub theme: Theme,
    watcher: ThemeWatcher,
    pub muted: Vec<String>,
    pub volume: f32,
    pub message: Option<(String, bool, Instant)>,
    pub mode: Mode,
    pub quit: bool,
    /// Height of the editor view, recorded at draw time for paging/scrolling.
    pub editor_height: usize,
}

impl App {
    pub fn new(path: Option<PathBuf>) -> App {
        let engine = Engine::start();
        let mut app = App {
            editor: Editor::new(loom_app::WELCOME),
            path: None,
            engine,
            program: Program::parse(""),
            running_text: None,
            theme: Theme::load(),
            watcher: ThemeWatcher::default(),
            muted: Vec::new(),
            volume: 0.8,
            message: None,
            mode: Mode::Edit,
            quit: false,
            editor_height: 20,
        };
        match &app.engine {
            Ok(e) => app.info(format!("son : {} · {} Hz — Ctrl+R pour jouer, F1 pour l'aide", e.device_name(), e.sample_rate())),
            Err(err) => app.error(format!("pas de son : {err}")),
        }
        if let Some(p) = path.or_else(Library::last_file) { app.open(p); }
        app
    }

    pub fn info(&mut self, msg: impl Into<String>) { self.message = Some((msg.into(), false, Instant::now())); }
    pub fn error(&mut self, msg: impl Into<String>) { self.message = Some((msg.into(), true, Instant::now())); }

    pub fn file_label(&self) -> String {
        let name = self.path.as_ref()
            .and_then(|p| p.file_name())
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| "sans titre".into());
        if self.editor.dirty { format!("{name} ●") } else { name }
    }

    /// True when the editor holds edits that aren't playing yet.
    pub fn out_of_sync(&self) -> bool {
        self.running_text.as_deref().is_some_and(|t| t != self.editor.text())
    }

    /// Called every frame: follow live Omarchy theme changes.
    pub fn tick(&mut self) {
        if let Some(t) = self.watcher.poll() {
            let name = t.name.clone();
            self.theme = t;
            self.info(format!("thème Omarchy : {name}"));
        }
    }

    // ---- actions -------------------------------------------------------

    pub fn run(&mut self) {
        let text = self.editor.text();
        let program = Program::parse(&text);
        if program.voices.is_empty() {
            self.error("aucune voix trouvée — une ligne ressemble à  kick: x . x .");
            return;
        }
        let Ok(engine) = &self.engine else { self.error("pas de sortie audio"); return };
        let was_playing = engine.status().playing();
        engine.run(&text);
        let n = program.voices.len();
        self.program = program;
        self.running_text = Some(text);
        self.info(if was_playing { format!("▶ {n} voix — change à la prochaine mesure") } else { format!("▶ {n} voix") });
    }

    pub fn stop(&mut self) {
        if let Ok(e) = &self.engine { e.stop(); }
        self.info("■ stop");
    }

    pub fn open(&mut self, path: PathBuf) {
        match std::fs::read_to_string(&path) {
            Ok(text) => {
                self.editor = Editor::new(&text);
                Library::remember(&path);
                self.info(format!("ouvert : {}", path.display()));
                self.path = Some(path);
            }
            Err(e) => self.error(format!("impossible d'ouvrir {} : {e}", path.display())),
        }
    }

    pub fn new_song(&mut self) {
        self.editor = Editor::new(loom_app::WELCOME);
        self.path = None;
        self.info("nouveau morceau");
    }

    pub fn save(&mut self) {
        let path = self.path.clone().unwrap_or_else(Library::new_song_path);
        match Library::save(&path, &self.editor.text()) {
            Ok(()) => {
                self.editor.dirty = false;
                Library::remember(&path);
                self.info(format!("enregistré : {}", path.display()));
                self.path = Some(path);
            }
            Err(e) => self.error(format!("échec de l'enregistrement : {e}")),
        }
    }

    /// Bounce the song to a WAV next to it (8 bars, or the whole `song:`).
    pub fn export(&mut self) {
        let text = self.editor.text();
        let program = Program::parse(&text);
        let bars = if program.len_bars() > 0 { program.len_bars() } else { 8 };
        let path = self.path.clone().unwrap_or_else(Library::new_song_path).with_extension("wav");
        let samples = loom_audio::render(&text, bars, 48000);
        let result = path.parent().map_or(Ok(()), std::fs::create_dir_all)
            .and_then(|_| loom_audio::write_wav(&path, &samples, 48000));
        match result {
            Ok(()) => self.info(format!("exporté {bars} mesures → {}", path.display())),
            Err(e) => self.error(format!("échec de l'export : {e}")),
        }
    }

    /// Mute/unmute the n-th voice (1-based, as numbered on the grid).
    pub fn toggle_mute(&mut self, n: usize) {
        let mut names: Vec<String> = Vec::new();
        for v in &self.program.voices { if !names.contains(&v.name) { names.push(v.name.clone()); } }
        let Some(name) = names.get(n.wrapping_sub(1)).cloned() else { return };
        let muted = !self.muted.contains(&name);
        if muted { self.muted.push(name.clone()) } else { self.muted.retain(|m| *m != name) }
        if let Ok(e) = &self.engine { e.set_muted(&name, muted); }
        self.info(format!("{name} {}", if muted { "coupé" } else { "rétabli" }));
    }

    pub fn nudge_volume(&mut self, delta: f32) {
        self.volume = (self.volume + delta).clamp(0.0, 1.5);
        if let Ok(e) = &self.engine { e.set_volume(self.volume); }
        self.info(format!("volume {:.0}%", self.volume * 100.0));
    }

    fn open_browser(&mut self) {
        let items: Vec<(String, PathBuf)> = Library::scan().into_iter()
            .flat_map(|(label, files)| files.into_iter().map(move |f| (label.clone(), f)))
            .collect();
        if items.is_empty() {
            self.error(format!("aucun .loom trouvé — enregistre un morceau (Ctrl+S) dans {}", Library::user_dir().display()));
            return;
        }
        let selected = self.path.as_ref().and_then(|p| items.iter().position(|(_, f)| f == p)).unwrap_or(0);
        self.mode = Mode::Browser { items, selected };
    }

    /// Run `action` now, or ask first if it would throw away unsaved edits.
    fn guarded(&mut self, action: Pending) {
        if self.editor.dirty {
            let question = match &action {
                Pending::Quit => "Quitter sans enregistrer ?",
                Pending::Open(_) => "Ouvrir sans enregistrer les changements ?",
                Pending::New => "Nouveau morceau sans enregistrer ?",
            }.to_string();
            self.mode = Mode::Confirm { question, then: action };
        } else {
            self.perform(action);
        }
    }

    fn perform(&mut self, action: Pending) {
        self.mode = Mode::Edit;
        match action {
            Pending::Quit => self.quit = true,
            Pending::Open(p) => self.open(p),
            Pending::New => self.new_song(),
        }
    }

    // ---- keys ----------------------------------------------------------

    pub fn on_key(&mut self, key: KeyEvent) {
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        let alt = key.modifiers.contains(KeyModifiers::ALT);
        match &mut self.mode {
            Mode::Help => { self.mode = Mode::Edit; return; }
            Mode::Confirm { then, .. } => {
                match key.code {
                    KeyCode::Char('o' | 'O' | 'y' | 'Y') | KeyCode::Enter => { let a = then.clone(); self.perform(a); }
                    _ => self.mode = Mode::Edit,
                }
                return;
            }
            Mode::Browser { items, selected } => {
                match key.code {
                    KeyCode::Up => *selected = selected.saturating_sub(1),
                    KeyCode::Down => *selected = (*selected + 1).min(items.len() - 1),
                    KeyCode::PageUp => *selected = selected.saturating_sub(10),
                    KeyCode::PageDown => *selected = (*selected + 10).min(items.len() - 1),
                    KeyCode::Enter => {
                        let path = items[*selected].1.clone();
                        self.mode = Mode::Edit;
                        self.guarded(Pending::Open(path));
                    }
                    _ => self.mode = Mode::Edit,
                }
                return;
            }
            Mode::Edit => {}
        }

        let page = self.editor_height.saturating_sub(1).max(1);
        match key.code {
            // transport & files
            KeyCode::Char('r') if ctrl => self.run(),
            KeyCode::F(5) => self.run(),
            KeyCode::Esc | KeyCode::F(6) => self.stop(),
            KeyCode::Char('s') if ctrl => self.save(),
            KeyCode::Char('o') if ctrl => self.open_browser(),
            KeyCode::Char('n') if ctrl => self.guarded(Pending::New),
            KeyCode::Char('e') if ctrl => self.export(),
            KeyCode::Char('q') if ctrl => self.guarded(Pending::Quit),
            KeyCode::F(1) => self.mode = Mode::Help,
            // mixer
            KeyCode::Char(c @ '1'..='9') if alt => self.toggle_mute(c as usize - '0' as usize),
            KeyCode::F(7) => self.nudge_volume(-0.05),
            KeyCode::F(8) => self.nudge_volume(0.05),
            // editing — terminals report Ctrl+/ as Ctrl+7 or Ctrl+_
            KeyCode::Char('/' | '7' | '_') if ctrl => self.editor.toggle_comment(),
            KeyCode::Char(c) if !ctrl && !alt => self.editor.insert_char(c),
            KeyCode::Enter => self.editor.newline(),
            KeyCode::Tab => self.editor.insert_str("  "),
            KeyCode::Backspace => self.editor.backspace(),
            KeyCode::Delete => self.editor.delete(),
            KeyCode::Left => self.editor.left(),
            KeyCode::Right => self.editor.right(),
            KeyCode::Up => self.editor.up(1),
            KeyCode::Down => self.editor.down(1),
            KeyCode::PageUp => self.editor.up(page),
            KeyCode::PageDown => self.editor.down(page),
            KeyCode::Home if ctrl => { self.editor.up(usize::MAX); self.editor.home(); }
            KeyCode::End if ctrl => { self.editor.down(usize::MAX); self.editor.end(); }
            KeyCode::Home => self.editor.home(),
            KeyCode::End => self.editor.end(),
            _ => {}
        }
    }

    /// Pasted text (bracketed paste) goes straight into the editor.
    pub fn on_paste(&mut self, text: &str) {
        if matches!(self.mode, Mode::Edit) { self.editor.insert_str(&text.replace("\r\n", "\n").replace('\r', "\n")); }
    }
}
