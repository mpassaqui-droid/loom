//! App state and keyboard handling for the terminal client.

use crate::editor::Editor;
use crate::scene::TermStage;
use loom_app::{Library, Theme, ThemeWatcher, VibeJob};
use loom_audio::Engine;
use loom_core::Program;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use std::path::PathBuf;
use std::time::Instant;

/// The studio's keys and per-frame work (mixer, timeline, TAP, stems, share, demo).
#[path = "app_studio.rs"]
mod studio_app;
pub use studio_app::{MixRow, Studio};

/// LE PROF & COMPOSITION: GÉNÉRER, DÉVELOPPE, undo, EXPLIQUE, ?question, OREILLE, HUM, LEÇONS.
#[path = "app_teacher.rs"]
mod teacher_app;
pub use teacher_app::Teacher;

pub enum Mode {
    Edit,
    /// The keyboard help (F1), scrolled by this many lines.
    Help { scroll: usize },
    /// The song picker: flattened `(group label, file)` entries and the selection.
    Browser { items: Vec<(String, PathBuf)>, selected: usize },
    /// The one-line VIBE prompt at the bottom: describe the music in words.
    Vibe { input: String },
    /// The full-screen SCÈNE (F12): visuals that follow the music.
    Stage,
    /// Unsaved changes would be lost: what to do if the user confirms.
    Confirm { question: String, then: Pending },
    /// The studio's mixer (Alt+M): one row per voice, then the master knobs.
    Mixer { row: usize, param: loom_app::studio::mixer::Param },
    /// Editing the `song:` arrangement (Alt+A), with the selected section.
    Timeline { selected: usize },
    /// The one-line import prompt (Alt+I): a `LOOM1:` code, a link or LOOM code.
    Import { input: String },
    /// The teacher's answer (Alt+E, or `?question` in F2), scrolled by this many lines.
    Prof { scroll: usize },
    /// OREILLE (Alt+O): ear training.
    Ear,
    /// HUM (Alt+H): sing a melody.
    Hum,
    /// LEÇONS & RÉFÉRENCE (Alt+L): tab, search and the selected row.
    Lessons { tab: loom_app::teacher::browse::Tab, query: String, selected: usize },
}

#[derive(Clone)]
pub enum Pending { Quit, Open(PathBuf), New, Import(String) }

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
    /// The VIBE request Claude is working on, if any.
    pub vibe: Option<VibeJob>,
    /// Sending MIDI clock (F9) / following an external clock (F10).
    pub midi_clock: bool,
    pub midi_follow: bool,
    pub metronome: bool,
    /// The SCÈNE's state (scene choice, sound analysis, sparks).
    pub stage: TermStage,
    /// STUDIO: master knobs, mixer, meters, MIDI LEARN, stems, demo…
    pub studio: Studio,
    /// LE PROF & COMPOSITION: undo history, teacher's answer, OREILLE, HUM.
    pub teacher: Teacher,
}

impl App {
    pub fn new(path: Option<PathBuf>) -> App {
        let engine = Engine::start();
        let studio = Studio::new(engine.as_ref().map_or(48_000, |e| e.sample_rate()), 0.8);
        if let Ok(e) = &engine { studio.knobs.apply_all(e); }
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
            vibe: None,
            midi_clock: false,
            midi_follow: false,
            metronome: false,
            stage: TermStage::default(),
            studio,
            teacher: Teacher::default(),
        };
        match &app.engine {
            Ok(e) => app.info(format!("son : {} · {} Hz · {} samples · {} — Ctrl+R pour jouer, F1 pour l'aide",
                e.device_name(), e.sample_rate(), e.sample_count(), e.midi_description())),
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

    /// Called every frame: follow live Omarchy theme changes, collect VIBE's answer.
    pub fn tick(&mut self) {
        if let Some(t) = self.watcher.poll() {
            let name = t.name.clone();
            self.theme = t;
            self.info(format!("thème Omarchy : {name}"));
        }
        self.poll_vibe();
        self.studio_tick();
        self.teacher_tick();
        // the stage listens while it's shown or the music plays (so old notes don't pile up)
        let playing = self.is_playing();
        if playing || matches!(self.mode, Mode::Stage) {
            self.stage.update(self.engine.as_ref().ok().map(|e| e.status()), &self.program);
        }
    }

    // ---- actions -------------------------------------------------------

    pub fn run(&mut self) {
        self.forget_demo(); // the song takes the engine back from a teacher's example
        let text = self.editor.text();
        let program = Program::parse(&text);
        if program.voices.is_empty() {
            self.error("aucune voix trouvée — une ligne ressemble à  kick: x . x .");
            return;
        }
        let Ok(engine) = &self.engine else { self.error("pas de sortie audio"); return };
        let was_playing = engine.status().playing();
        engine.run(&self.engine_code(&text));
        self.studio.mixer.sync(&program);
        self.studio.mixer.apply(engine);
        let n = program.voices.len();
        self.stage.load(&text, &program);
        self.program = program;
        self.running_text = Some(text);
        self.info(if was_playing { format!("▶ {n} voix — change à la prochaine mesure") } else { format!("▶ {n} voix") });
    }

    pub fn stop(&mut self) {
        self.forget_demo();
        if let Ok(e) = &self.engine { e.stop(); }
        self.info("■ stop");
    }

    pub fn open(&mut self, path: PathBuf) {
        match std::fs::read_to_string(&path) {
            Ok(text) => {
                self.forget_vibe();
                self.forget_song_settings();
                self.editor = Editor::new(&text);
                self.stage.load(&text, &Program::parse(&text));
                Library::remember(&path);
                self.info(format!("ouvert : {}", path.display()));
                self.path = Some(path);
            }
            Err(e) => self.error(format!("impossible d'ouvrir {} : {e}", path.display())),
        }
    }

    pub fn new_song(&mut self) {
        self.forget_vibe();
        self.forget_song_settings();
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

    /// F9: send MIDI clock so a DAW or drum machine follows LOOM's tempo.
    pub fn toggle_midi_clock(&mut self) {
        let Ok(e) = &self.engine else { return };
        self.midi_clock = !self.midi_clock;
        e.set_midi_clock(self.midi_clock);
        let msg = if self.midi_clock { format!("horloge MIDI envoyée · {}", e.midi_description()) } else { "horloge MIDI coupée".into() };
        self.info(msg);
    }

    /// F10: let an external MIDI clock drive LOOM (tempo, start, stop).
    pub fn toggle_midi_follow(&mut self) {
        let Ok(e) = &self.engine else { return };
        self.midi_follow = !self.midi_follow;
        e.set_midi_follow(self.midi_follow);
        self.info(if self.midi_follow { "LOOM suit l'horloge MIDI externe" } else { "tempo du morceau rétabli" });
    }

    /// Ctrl+T: tick on every beat.
    pub fn toggle_metronome(&mut self) {
        let Ok(e) = &self.engine else { return };
        self.metronome = !self.metronome;
        e.set_metronome(self.metronome);
        self.info(if self.metronome { "métronome activé" } else { "métronome coupé" });
    }

    /// F4: record everything that plays (live edits included) into a WAV.
    pub fn toggle_recording(&mut self) {
        let Ok(e) = &self.engine else { return };
        let result = if e.is_recording() {
            e.stop_recording().map(|p| format!("● enregistrement terminé → {}", p.display()))
        } else {
            let path = Library::new_recording_path(self.path.as_deref());
            e.start_recording(&path).map(|_| format!("● REC → {} (F4 pour arrêter)", path.display()))
        };
        match result { Ok(m) => self.info(m), Err(err) => self.error(format!("REC : {err}")) }
    }

    pub fn is_recording(&self) -> bool { self.engine.as_ref().is_ok_and(|e| e.is_recording()) }

    pub fn nudge_volume(&mut self, delta: f32) {
        self.volume = (self.volume + delta).clamp(0.0, 1.5);
        if let Ok(e) = &self.engine { e.set_volume(self.volume); }
        self.studio.knobs.set(loom_app::studio::knobs::Knob::Volume, self.volume * 100.0);
        self.info(format!("volume {:.0}%", self.volume * 100.0));
    }

    // ---- VIBE ----------------------------------------------------------

    /// Send the words typed in the VIBE prompt to Claude, with the current code
    /// (`?question` goes to the teacher instead).
    fn send_vibe(&mut self, words: &str) {
        if words.trim().is_empty() {
            self.error("VIBE : écris d'abord ce que tu veux, ex.  fais un groove afro plus sombre  (ou ?quinte pour le prof)");
            return;
        }
        let Some(words) = self.route_prompt(words) else { return };
        self.vibe = Some(VibeJob::start(&words, &self.editor.text()));
    }

    /// Esc while Claude thinks: stop waiting (the `claude` process is killed).
    fn cancel_vibe(&mut self) {
        if self.vibe.take().is_some() { self.info("VIBE annulé — rien n'a changé"); }
    }

    /// Put VIBE's code in the editor once it's ready (checked by the parser).
    /// While playing it goes straight to the engine, at the next bar.
    fn poll_vibe(&mut self) {
        let Some(result) = self.vibe.as_mut().and_then(VibeJob::poll) else { return };
        let words = self.vibe.take().map(|j| j.words.clone()).unwrap_or_default();
        match result {
            Ok(answer) => {
                let when = self.replace_song("VIBE", &answer.code);
                self.info(format!("VIBE ok · {}{when} · F3 annule // {words}", answer.summary));
            }
            Err(e) => self.error(format!("VIBE : {e}")),
        }
    }

    /// A VIBE in progress or the undo history belongs to the song being replaced.
    fn forget_vibe(&mut self) {
        self.vibe = None;
        self.teacher.history.clear();
    }

    pub fn is_playing(&self) -> bool {
        self.engine.as_ref().is_ok_and(|e| e.status().playing())
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
    pub(crate) fn guarded(&mut self, action: Pending) {
        if self.editor.dirty {
            let question = match &action {
                Pending::Quit => "Quitter sans enregistrer ?",
                Pending::Open(_) => "Ouvrir sans enregistrer les changements ?",
                Pending::New => "Nouveau morceau sans enregistrer ?",
                Pending::Import(_) => "Importer sans enregistrer les changements ?",
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
            Pending::Import(text) => self.import(&text),
        }
    }

    // ---- keys ----------------------------------------------------------

    pub fn on_key(&mut self, key: KeyEvent) {
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        let alt = key.modifiers.contains(KeyModifiers::ALT);
        if self.studio_key(key) { return; }
        if self.teacher_key(key) { return; }
        match &mut self.mode {
            Mode::Help { scroll } => {
                match key.code {
                    KeyCode::Up => *scroll = scroll.saturating_sub(1),
                    KeyCode::Down => *scroll += 1,
                    KeyCode::PageUp => *scroll = scroll.saturating_sub(10),
                    KeyCode::PageDown => *scroll += 10,
                    _ => self.mode = Mode::Edit,
                }
                return;
            }
            Mode::Stage => {
                match key.code {
                    KeyCode::Esc | KeyCode::F(12) => { self.mode = Mode::Edit; return; }
                    KeyCode::Char('n' | 'N') if !ctrl && !alt => { self.stage.next_scene(); return; }
                    KeyCode::Char('v' | 'V') if !ctrl && !alt => {
                        self.stage.director.vj = !self.stage.director.vj;
                        self.info(if self.stage.director.vj { "mode VJ : la scène change toutes les 4 mesures" } else { "mode VJ coupé" });
                        return;
                    }
                    // no typing into the hidden editor; transport keys (Ctrl+R, F6, F7…) still work
                    KeyCode::Char(_) if !ctrl => return,
                    KeyCode::Enter | KeyCode::Tab | KeyCode::Backspace | KeyCode::Delete | KeyCode::Up | KeyCode::Down
                    | KeyCode::Left | KeyCode::Right | KeyCode::PageUp | KeyCode::PageDown | KeyCode::Home | KeyCode::End
                    | KeyCode::F(1) | KeyCode::F(2) => return,
                    _ => {}
                }
            }
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
            Mode::Vibe { input } => {
                match key.code {
                    KeyCode::Enter => {
                        let words = std::mem::take(input);
                        self.mode = Mode::Edit;
                        self.send_vibe(&words);
                    }
                    KeyCode::Esc | KeyCode::F(2) => self.mode = Mode::Edit,
                    KeyCode::Backspace => { input.pop(); }
                    KeyCode::Char('u') if ctrl => input.clear(),
                    KeyCode::Char(c) if !ctrl && !alt => input.push(c),
                    _ => {}
                }
                return;
            }
            Mode::Edit | Mode::Mixer { .. } | Mode::Timeline { .. } | Mode::Import { .. }
            | Mode::Prof { .. } | Mode::Ear | Mode::Hum | Mode::Lessons { .. } => {}
        }

        let page = self.editor_height.saturating_sub(1).max(1);
        match key.code {
            // transport & files
            KeyCode::Char('r') if ctrl => self.run(),
            KeyCode::F(5) => self.run(),
            // Esc first cancels a VIBE that is thinking, then stops the music
            KeyCode::Esc if self.vibe.is_some() => self.cancel_vibe(),
            KeyCode::Esc | KeyCode::F(6) => self.stop(),
            KeyCode::Char('s') if ctrl => self.save(),
            KeyCode::Char('o') if ctrl => self.open_browser(),
            KeyCode::Char('n') if ctrl => self.guarded(Pending::New),
            KeyCode::Char('e') if ctrl => self.export(),
            KeyCode::Char('q') if ctrl => self.guarded(Pending::Quit),
            KeyCode::F(1) => self.mode = Mode::Help { scroll: 0 },
            KeyCode::F(12) => self.mode = Mode::Stage,
            // VIBE: describe it, Claude writes the code
            KeyCode::F(2) if self.vibe.is_some() => self.error("VIBE réfléchit encore — Échap pour annuler"),
            KeyCode::F(2) => self.mode = Mode::Vibe { input: String::new() },
            // mixer
            KeyCode::Char(c @ '1'..='9') if alt => self.toggle_mute(c as usize - '0' as usize),
            KeyCode::F(4) => self.toggle_recording(),
            KeyCode::Char('t') if ctrl => self.toggle_metronome(),
            KeyCode::F(9) => self.toggle_midi_clock(),
            KeyCode::F(10) => self.toggle_midi_follow(),
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
        match &mut self.mode {
            Mode::Edit => self.editor.insert_str(&text.replace("\r\n", "\n").replace('\r', "\n")),
            Mode::Vibe { input } => input.push_str(&text.replace(['\r', '\n'], " ")),
            Mode::Import { input } => input.push_str(&text.replace("\r\n", "\n").replace('\r', "\n")),
            Mode::Lessons { query, .. } => query.push_str(&text.replace(['\r', '\n'], " ")),
            Mode::Ear => if let Some(ear) = &mut self.teacher.ear { ear.input.push_str(text.trim()) },
            _ => {}
        }
    }
}
