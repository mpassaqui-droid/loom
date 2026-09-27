//! The LOOM window: its state, and what happens each frame.
//!
//! Each frame the UI is drawn top to bottom (transport bar, VIBE bar, status
//! bar, library, grid with the stage thumbnail, editor), or, in SCÈNE mode
//! (F11), the full-window stage instead. Drawing never changes the song directly: buttons
//! and keyboard shortcuts produce [`Action`]s, which [`LoomApp::perform`]
//! carries out once the frame is drawn. That keeps "what the user asked for"
//! in one place.

use std::path::{Path, PathBuf};
use std::sync::mpsc::Receiver;
use std::time::Duration;

use eframe::egui::{self, Align, Button, Id, Key, Layout, Modifiers, RichText, Sense};
use loom_app::{Library, Theme, ThemeWatcher, VibeJob, WELCOME};
use loom_audio::Engine;
use loom_core::Program;

use crate::editor::Highlighter;
use crate::export::{self, ExportResult};
use crate::grid;
use crate::scene::{Look, Overlay, Stage};
use crate::studio_ui::{self, Studio, StudioAction};
use crate::teacher_ui::{Teacher, TeacherAction};
use crate::style::{self, color};

/// The studio's part of the app (actions, per-frame work, panels).
#[path = "app_studio.rs"]
mod studio;

/// LE PROF & COMPOSITION's part of the app (GÉNÉRER, DÉVELOPPE, undo, EXPLIQUE, OREILLE, HUM, LEÇONS).
#[path = "app_teacher.rs"]
mod teacher;

/// Widget id of the VIBE text field (Ctrl+G / F2 focus it).
const VIBE_FIELD: &str = "loom-vibe";
/// Widget id of the code editor (the piano roll moves its cursor).
const EDITOR_ID: &str = "loom-editor";

/// Something the user asked for, with a button or a shortcut.
#[derive(Debug, Clone, PartialEq)]
pub enum Action {
    Run,
    Stop,
    Save,
    New,
    Export,
    Open(PathBuf),
    ToggleMute(String),
    RefreshLibrary,
    /// Send the VIBE field's words to Claude.
    Vibe,
    /// Stop waiting for Claude.
    CancelVibe,
    /// Start or stop recording what plays into a WAV (F4).
    Record,
    /// Tick on every beat (Ctrl+T).
    Metronome,
    /// Show / hide the full-window SCÈNE (F11, Ctrl+F; Échap leaves it).
    ToggleStage,
    /// Next scene (N on the stage).
    NextScene,
    /// Auto-change the scene every 4 bars (V on the stage).
    ToggleVj,
    /// Pure-visual mode: hide every letter on the stage (T on the stage).
    ToggleStageText,
    /// Close the window.
    Quit,
    /// Toggle the large-text accessibility mode (Ctrl+Shift+A).
    ToggleUiSize,
    /// A STUDIO button or key: knobs, mixer, TAP, STEMS, share, timeline… (see `studio_ui`).
    Studio(StudioAction),
    /// A PROF button or key: GÉNÉRER, DÉVELOPPE, undo, EXPLIQUE, OREILLE, HUM, LEÇONS (see `teacher_ui`).
    Teacher(TeacherAction),
}

impl Action {
    /// Would this throw away the editor text (and its unsaved edits)?
    fn loses_edits(&self) -> bool {
        matches!(
            self,
            Action::New | Action::Open(_) | Action::Quit | Action::Studio(StudioAction::Import(_) | StudioAction::Launch(_))
        )
    }
}

/// The last thing to tell the user, shown in the status bar.
struct Message {
    text: String,
    is_error: bool,
}

pub struct LoomApp {
    // sound
    engine: Option<Engine>,
    /// Why there is no engine (no sound card…), kept for the status bar.
    audio_error: Option<String>,
    muted: Vec<String>,

    // the song in the editor
    text: String,
    /// The text as it is on disk, to know whether there are unsaved edits.
    saved_text: String,
    path: Option<PathBuf>,
    /// What the grid shows: the code last RUN (or the editor text while stopped).
    program: Program,

    // look
    theme: Theme,
    theme_watcher: ThemeWatcher,
    highlighter: Highlighter,

    // library, dialogs, background work
    library: Vec<(String, Vec<PathBuf>)>,
    message: Message,
    /// An action waiting for "discard unsaved changes?" to be answered.
    confirm: Option<Action>,
    export: Option<Receiver<ExportResult>>,

    // VIBE: describe the music, Claude writes the code
    vibe_input: String,
    vibe: Option<VibeJob>,
    /// What the last VIBE changed, e.g. `+lead ~bass`, shown next to the field.
    vibe_summary: Option<String>,
    first_frame: bool,
    /// Set once the user agreed to close, so the next close request goes through.
    closing: bool,
    /// Sending MIDI clock / following an external MIDI clock.
    midi_clock: bool,
    midi_follow: bool,
    metronome: bool,

    // SCÈNE: visuals that follow the music
    stage: Stage,
    /// True while the stage fills the window instead of the editor.
    stage_full: bool,
    /// Draw the stage's text (title, section, bar, hints, scene name). Off by
    /// default — the stage is a pure-visual projection; T brings the text back.
    /// Persisted, so a restart keeps the choice.
    stage_text: bool,
    /// Large-text accessibility mode (persisted, toggled with Ctrl+Shift+A).
    ui_large: bool,

    // STUDIO: master knobs (VOL included), mixer, meters, MIDI LEARN, demo…
    studio: Studio,

    // LE PROF & COMPOSITION: undo history, teacher's answer, OREILLE, HUM, LEÇONS
    teacher: Teacher,
}

impl LoomApp {
    /// Open the sound card, load the theme and the first song.
    /// `file` is the command-line argument; without it, the last song reopens.
    pub fn new(cc: &eframe::CreationContext<'_>, file: Option<PathBuf>) -> LoomApp {
        let theme = Theme::load();
        let ui_large = Theme::ui_large();
        style::apply_text_sizes(&cc.egui_ctx, ui_large);
        style::apply(&cc.egui_ctx, &theme);

        let (engine, audio_error) = match Engine::start() {
            Ok(engine) => (Some(engine), None),
            Err(e) => (None, Some(e)),
        };
        let studio = Studio::new(engine.as_ref().map_or(48_000, |e| e.sample_rate()));
        if let Some(engine) = &engine {
            studio.knobs.apply_all(engine);
        }

        let mut app = LoomApp {
            engine,
            audio_error,
            muted: Vec::new(),
            text: WELCOME.to_string(),
            saved_text: WELCOME.to_string(),
            path: None,
            program: Program::parse(WELCOME),
            theme,
            theme_watcher: ThemeWatcher::default(),
            highlighter: Highlighter::default(),
            library: Library::scan(),
            message: Message { text: String::new(), is_error: false },
            confirm: None,
            export: None,
            vibe_input: String::new(),
            vibe: None,
            vibe_summary: None,
            first_frame: true,
            closing: false,
            midi_clock: false,
            midi_follow: false,
            metronome: false,
            stage: Stage::default(),
            stage_full: false,
            stage_text: Theme::stage_text(),
            ui_large,
            studio,
            teacher: Teacher::default(),
        };
        app.stage.load(WELCOME, &app.program);
        app.startup_message();

        match file {
            Some(path) if path.exists() => app.open(&path),
            Some(path) => {
                // `loom-gui nouveau.loom`: start that song from the welcome pattern
                app.saved_text = String::new();
                app.say(format!("Nouveau fichier : {} (Ctrl+S pour le créer)", path.display()));
                app.path = Some(path);
            }
            None => {
                if let Some(last) = Library::last_file() {
                    app.open(&last);
                }
            }
        }
        app
    }

    fn startup_message(&mut self) {
        match (&self.engine, &self.audio_error) {
            (Some(engine), _) => {
                let text = format!("Son prêt : {} ({} Hz), {} samples, {}. Ctrl+Entrée pour jouer.",
                    engine.device_name(), engine.sample_rate(), engine.sample_count(), engine.midi_description());
                self.say(text);
            }
            (None, Some(e)) => self.warn(format!("Pas de son : {e}. L'éditeur et l'export WAV marchent quand même.")),
            (None, None) => {}
        }
    }

    // ---------------------------------------------------------------- state

    fn is_modified(&self) -> bool {
        self.text != self.saved_text
    }

    fn is_playing(&self) -> bool {
        self.engine.as_ref().is_some_and(|e| e.status().playing())
    }

    fn file_name(&self) -> String {
        self.path
            .as_ref()
            .and_then(|p| p.file_name())
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| "sans titre".into())
    }

    fn say(&mut self, text: impl Into<String>) {
        self.message = Message { text: text.into(), is_error: false };
    }

    fn warn(&mut self, text: impl Into<String>) {
        self.message = Message { text: text.into(), is_error: true };
    }

    // -------------------------------------------------------------- actions

    /// Carry out an action, asking first if it would throw away unsaved edits.
    fn perform(&mut self, action: Action, ctx: &egui::Context) {
        if matches!(action, Action::Studio(StudioAction::Import(_))) {
            self.studio.import = None; // the "unsaved changes" question replaces the import box
        }
        if action.loses_edits() && self.is_modified() {
            self.confirm = Some(action);
        } else {
            self.perform_now(action, ctx);
        }
    }

    fn perform_now(&mut self, action: Action, ctx: &egui::Context) {
        match action {
            Action::Run => self.run(),
            Action::Stop => self.stop(),
            Action::Save => {
                self.save();
            }
            Action::New => self.new_song(),
            Action::Export => self.export(ctx),
            Action::Open(path) => self.open(&path),
            Action::ToggleMute(voice) => self.toggle_mute(voice),
            Action::RefreshLibrary => {
                self.library = Library::scan();
                self.say("Bibliothèque actualisée.");
            }
            Action::Vibe => self.send_vibe(),
            Action::CancelVibe => {
                if self.vibe.take().is_some() {
                    self.say("VIBE annulé : rien n'a changé.");
                }
            }
            Action::Record => self.toggle_recording(),
            Action::Metronome => {
                if let Some(engine) = &self.engine {
                    self.metronome = !self.metronome;
                    engine.set_metronome(self.metronome);
                    self.say(if self.metronome { "Métronome activé." } else { "Métronome coupé." });
                }
            }
            Action::ToggleStage => {
                self.stage_full = !self.stage_full;
                if self.stage_full {
                    self.say("Scène : N change de scène, V active le mode VJ, T affiche ou cache le texte, Échap revient au code.");
                }
            }
            Action::NextScene => self.stage.next_scene(),
            Action::ToggleVj => {
                self.stage.toggle_vj();
                self.say(if self.stage.director.vj { "Mode VJ : la scène change toutes les 4 mesures." } else { "Mode VJ coupé." });
            }
            Action::ToggleStageText => {
                self.stage_text = !self.stage_text;
                Theme::set_stage_text(self.stage_text);
                self.say(if self.stage_text { "Texte de la scène affiché (T pour le cacher)." } else { "Mode pur visuel : plus aucun texte." });
            }
            Action::Quit => {
                self.closing = true;
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
            Action::ToggleUiSize => {
                self.ui_large = !self.ui_large;
                Theme::set_ui_large(self.ui_large);
                style::apply_text_sizes(ctx, self.ui_large);
                self.say(if self.ui_large {
                    "Grande police activée (accessibilité)."
                } else {
                    "Taille normale rétablie."
                });
                ctx.request_repaint();
            }
            Action::Studio(action) => self.studio_action(action, ctx),
            Action::Teacher(action) => self.teacher_action(action),
        }
    }

    /// RUN: send the code to the engine. Starts playing if stopped; while
    /// playing, the change lands on the next bar line.
    fn run(&mut self) {
        self.forget_demo(); // the song takes the engine back from a teacher's example
        let program = Program::parse(&self.text);
        let voice_count = program.voices.len();
        let Some(engine) = &self.engine else {
            self.set_program(program);
            self.warn("Pas de sortie audio : impossible de jouer (l'export WAV reste possible).");
            return;
        };
        let was_playing = engine.status().playing();
        engine.run(&self.engine_code());
        self.studio.mixer.sync(&program);
        self.studio.mixer.apply(engine);
        self.set_program(program);
        if voice_count == 0 {
            self.warn("Aucune voix reconnue. Une voix s'écrit par exemple  kick: x . x .");
        } else if was_playing {
            self.say(format!("Code envoyé ({voice_count} voix) : il démarre à la prochaine mesure."));
        } else {
            self.say(format!("Lecture ({voice_count} voix)."));
        }
    }

    fn stop(&mut self) {
        self.forget_demo();
        if let Some(engine) = &self.engine {
            engine.stop();
            self.say("Arrêt.");
        }
    }

    /// Save to the current file, or to a new file in the song folder.
    /// Returns false if it failed (the error is in the status bar).
    fn save(&mut self) -> bool {
        let path = self.path.clone().unwrap_or_else(Library::new_song_path);
        match Library::save(&path, &self.text) {
            Ok(()) => {
                let path = path.canonicalize().unwrap_or(path);
                Library::remember(&path);
                self.saved_text = self.text.clone();
                self.say(format!("Enregistré : {}", path.display()));
                self.path = Some(path);
                self.library = Library::scan();
                true
            }
            Err(e) => {
                self.warn(format!("Impossible d'enregistrer {} : {e}", path.display()));
                false
            }
        }
    }

    fn new_song(&mut self) {
        self.set_text(WELCOME.to_string(), None);
        self.say("Nouveau morceau (Ctrl+S pour l'enregistrer).");
    }

    fn open(&mut self, path: &Path) {
        match std::fs::read_to_string(path) {
            Ok(text) => {
                let path = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
                Library::remember(&path);
                self.set_text(text, Some(path));
                self.say(format!("Ouvert : {}  (Ctrl+Entrée pour l'entendre)", self.file_name()));
            }
            Err(e) => self.warn(format!("Impossible d'ouvrir {} : {e}", path.display())),
        }
    }

    /// Replace the editor content with a freshly opened (unmodified) song.
    /// Any VIBE in progress or undo belongs to the old song, so it's dropped.
    fn set_text(&mut self, text: String, path: Option<PathBuf>) {
        self.vibe = None;
        self.teacher.history.clear();
        self.vibe_summary = None;
        // mixer settings, clips and the timeline selection belong to the old song
        self.studio.mixer.reset(self.engine.as_ref());
        self.studio.clips.clear();
        self.studio.section = None;
        self.saved_text = text.clone();
        self.text = text;
        self.path = path;
        if !self.is_playing() {
            self.set_program(Program::parse(&self.text));
        }
    }

    /// What the grid and the stage show: the code last RUN (or, while
    /// stopped, the editor's). The stage reads its `scene:` lines from it.
    fn set_program(&mut self, program: Program) {
        self.stage.load(&self.text, &program);
        self.studio.mixer.sync(&program);
        self.program = program;
    }

    /// REC: everything that plays (live edits included) goes to a WAV in
    /// `~/Music/loom/enregistrements`.
    fn toggle_recording(&mut self) {
        let Some(engine) = &self.engine else {
            self.warn("Pas de sortie audio : rien à enregistrer.");
            return;
        };
        let result = if engine.is_recording() {
            engine.stop_recording().map(|p| format!("Enregistrement terminé : {}", p.display()))
        } else {
            let path = Library::new_recording_path(self.path.as_deref());
            engine.start_recording(&path).map(|_| format!("● REC en cours → {} (F4 pour arrêter)", path.display()))
        };
        match result {
            Ok(m) => self.say(m),
            Err(e) => self.warn(format!("REC : {e}")),
        }
    }

    fn export(&mut self, ctx: &egui::Context) {
        if self.export.is_some() {
            return;
        }
        let bars = export::bars_to_export(&Program::parse(&self.text));
        let path = export::wav_path(self.path.as_deref());
        let sample_rate = self.engine.as_ref().map_or(48_000, |e| e.sample_rate());
        self.export = Some(export::spawn(self.text.clone(), bars, path.clone(), sample_rate, ctx.clone()));
        self.say(format!("Export de {bars} mesures vers {}…", path.display()));
    }

    /// Show the export result once the background thread is done.
    fn poll_export(&mut self) {
        let Some(rx) = &self.export else { return };
        match rx.try_recv() {
            Ok(Ok(path)) => {
                self.export = None;
                self.say(format!("WAV exporté : {}", path.display()));
            }
            Ok(Err(e)) => {
                self.export = None;
                self.warn(e);
            }
            Err(std::sync::mpsc::TryRecvError::Empty) => {}
            Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                self.export = None;
                self.warn("L'export s'est interrompu.");
            }
        }
    }

    /// Ask Claude to apply the VIBE field's words to the current code
    /// (`?question` goes to the teacher instead).
    fn send_vibe(&mut self) {
        if self.vibe.is_some() {
            return;
        }
        if self.vibe_input.trim().is_empty() {
            self.warn("VIBE : écris d'abord ce que tu veux, par exemple « fais un groove afro plus sombre », ou ?quinte pour le prof.");
            return;
        }
        let Some(words) = self.route_vibe_input() else { return };
        self.vibe = Some(VibeJob::start(&words, &self.text));
        self.say(format!("VIBE réfléchit à « {words} »…"));
    }

    /// Put VIBE's code in the editor once it's ready (already checked by the
    /// parser). While playing, it goes to the engine for the next bar.
    fn poll_vibe(&mut self) {
        let Some(result) = self.vibe.as_mut().and_then(VibeJob::poll) else { return };
        let words = self.vibe.take().map(|job| job.words.clone()).unwrap_or_default();
        match result {
            Ok(answer) => {
                let when = self.replace_song("VIBE", answer.code);
                self.say(format!("VIBE « {words} » : {} ({} voix){when}. F3 annule.", answer.summary, answer.voices));
                self.vibe_summary = Some(answer.summary);
                self.vibe_input.clear();
            }
            Err(e) => self.warn(format!("VIBE : {e}")),
        }
    }

    fn toggle_mute(&mut self, voice: String) {
        let muted = !self.muted.contains(&voice);
        if let Some(engine) = &self.engine {
            engine.set_muted(&voice, muted);
        }
        if muted {
            self.say(format!("{voice} coupé."));
            self.muted.push(voice);
        } else {
            self.say(format!("{voice} rétabli."));
            self.muted.retain(|m| *m != voice);
        }
    }

    // ----------------------------------------------------------------- keys

    /// Keyboard shortcuts. They are read (and removed) before the editor
    /// sees the keys, so Ctrl+Enter doesn't also type a new line.
    fn shortcuts(&mut self, ctx: &egui::Context) -> Vec<Action> {
        // closing during the demo: bring the user's song back first
        if ctx.input(|i| i.viewport().close_requested()) && self.studio.demo.is_some() {
            self.stop_demo();
        }
        // closing the window (Super+W, the × …) with unsaved edits: ask first
        if ctx.input(|i| i.viewport().close_requested()) && self.is_modified() && !self.closing {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            return vec![Action::Quit];
        }
        if self.confirm.is_some() {
            if ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Escape)) {
                self.confirm = None;
            }
            return Vec::new();
        }
        if let Some(stop_demo) = self.demo_interrupted(ctx) {
            return vec![stop_demo];
        }
        if self.studio.import.is_some() {
            return Vec::new(); // typing in the import box
        }
        if self.teacher.hum.is_some() {
            // the HUM dialog is modal: Escape closes it, nothing else reaches the song
            if ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Escape)) {
                return vec![Action::Teacher(TeacherAction::CloseHum)];
            }
            return Vec::new();
        }
        let vibe_running = self.vibe.is_some();
        let prof_running = self.teacher.prof_job.is_some();
        let stage_full = self.stage_full;
        let mut focus_vibe = false;
        let actions = ctx.input_mut(|i| {
            let mut actions = Vec::new();
            // most specific first: Ctrl+Shift+E before any plain Ctrl shortcut
            if i.consume_key(Modifiers::COMMAND | Modifiers::SHIFT, Key::E) {
                actions.push(Action::Export);
            }
            // LE PROF (Ctrl+Shift+G before Ctrl+G, Shift+F3 before F3)
            for (key, action) in [
                (Key::G, TeacherAction::Generate),
                (Key::B, TeacherAction::Develop),
                (Key::P, TeacherAction::Explain),
                (Key::O, TeacherAction::ToggleEar),
                (Key::H, TeacherAction::OpenHum),
            ] {
                if i.consume_key(Modifiers::COMMAND | Modifiers::SHIFT, key) {
                    actions.push(Action::Teacher(action));
                }
            }
            if i.consume_key(Modifiers::SHIFT, Key::F3) {
                actions.push(Action::Teacher(TeacherAction::Redo));
            }
            if i.consume_key(Modifiers::NONE, Key::F1) {
                actions.push(Action::Teacher(TeacherAction::ToggleHelp));
            }
            if i.consume_key(Modifiers::NONE, Key::F12) {
                actions.push(Action::Teacher(TeacherAction::ToggleLessons));
            }
            if i.consume_key(Modifiers::COMMAND, Key::Enter) {
                actions.push(Action::Run);
            }
            if i.consume_key(Modifiers::COMMAND, Key::Period) {
                actions.push(Action::Stop);
            }
            // Escape leaves the stage; in the editor it cancels a VIBE that
            // is thinking, otherwise stops the music
            if i.consume_key(Modifiers::NONE, Key::Escape) {
                actions.push(if stage_full {
                    Action::ToggleStage
                } else if vibe_running {
                    Action::CancelVibe
                } else if prof_running {
                    Action::Teacher(TeacherAction::CancelProf)
                } else {
                    Action::Stop
                });
            }
            if i.consume_key(Modifiers::NONE, Key::F11) || i.consume_key(Modifiers::COMMAND, Key::F) {
                actions.push(Action::ToggleStage);
            }
            // on the stage there is no text field, so plain letters are free
            if stage_full && i.consume_key(Modifiers::NONE, Key::N) {
                actions.push(Action::NextScene);
            }
            if stage_full && i.consume_key(Modifiers::NONE, Key::V) {
                actions.push(Action::ToggleVj);
            }
            if stage_full && i.consume_key(Modifiers::NONE, Key::T) {
                actions.push(Action::ToggleStageText);
            }
            if i.consume_key(Modifiers::COMMAND, Key::G) || i.consume_key(Modifiers::NONE, Key::F2) {
                focus_vibe = true;
            }
            if i.consume_key(Modifiers::NONE, Key::F3) {
                actions.push(Action::Teacher(TeacherAction::Undo));
            }
            if i.consume_key(Modifiers::NONE, Key::F4) {
                actions.push(Action::Record);
            }
            if i.consume_key(Modifiers::NONE, Key::F6) {
                actions.push(Action::Studio(StudioAction::TogglePanel));
            }
            if i.consume_key(Modifiers::NONE, Key::F7) {
                actions.push(Action::Studio(StudioAction::Tap));
            }
            if i.consume_key(Modifiers::NONE, Key::F8) {
                actions.push(Action::Studio(StudioAction::ToggleMixer));
            }
            if i.consume_key(Modifiers::NONE, Key::F9) {
                actions.push(Action::Studio(StudioAction::ToggleRoll));
            }
            if i.consume_key(Modifiers::NONE, Key::F10) {
                actions.push(Action::Studio(StudioAction::ToggleClips));
            }
            if i.consume_key(Modifiers::COMMAND | Modifiers::SHIFT, Key::L) {
                actions.push(Action::Studio(StudioAction::Share));
            }
            if i.consume_key(Modifiers::COMMAND | Modifiers::SHIFT, Key::I) {
                actions.push(Action::Studio(StudioAction::OpenImport));
            }
            if i.consume_key(Modifiers::COMMAND | Modifiers::SHIFT, Key::D) {
                actions.push(Action::Studio(StudioAction::ToggleDemo));
            }
            if i.consume_key(Modifiers::COMMAND, Key::T) {
                actions.push(Action::Metronome);
            }
            if i.consume_key(Modifiers::COMMAND | Modifiers::SHIFT, Key::A) {
                actions.push(Action::ToggleUiSize);
            }
            if i.consume_key(Modifiers::COMMAND, Key::S) {
                actions.push(Action::Save);
            }
            if i.consume_key(Modifiers::COMMAND, Key::N) {
                actions.push(Action::New);
            }
            actions
        });
        if focus_vibe {
            ctx.memory_mut(|m| m.request_focus(Id::new(VIBE_FIELD)));
        }
        actions
    }

    // ------------------------------------------------------------------- UI

    /// RUN / STOP, file name, bar counter, tempo, volume, meter, export.
    fn top_bar(&mut self, ui: &mut egui::Ui, actions: &mut Vec<Action>) {
        let theme = self.theme.clone();
        egui::Panel::top("transport")
            .frame(egui::Frame::new().fill(color(theme.panel)).inner_margin(8))
            .show(ui, |ui| {
                ui.horizontal_wrapped(|ui| {
                    ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Extend); // "120 BPM" stays on one line
                    let run = Button::new(RichText::new("▶ RUN").size(18.0).strong().color(color(theme.background)))
                        .fill(color(theme.accent))
                        .min_size(egui::vec2(100.0, 34.0));
                    if ui.add(run).on_hover_text("Jouer le code (Ctrl+Entrée)").clicked() {
                        actions.push(Action::Run);
                    }
                    let stop = Button::new(RichText::new("■ STOP").size(18.0).strong())
                        .min_size(egui::vec2(100.0, 34.0));
                    if ui.add(stop).on_hover_text("Arrêter (Échap ou Ctrl+.)").clicked() {
                        actions.push(Action::Stop);
                    }
                    ui.separator();

                    let modified = if self.is_modified() { " ⏺" } else { "" };
                    ui.label(RichText::new(format!("{}{modified}", self.file_name())).size(16.0).strong())
                        .on_hover_text(self.path.as_ref().map_or("pas encore enregistré".into(), |p| p.display().to_string()));
                    ui.separator();

                    let (position, bpm) = self.transport_text();
                    ui.label(RichText::new(position).monospace().color(color(theme.accent)));
                    ui.label(RichText::new(bpm).monospace());
                    ui.separator();

                    ui.label("Volume");
                    let volume = loom_app::studio::knobs::Knob::Volume;
                    let slider = egui::Slider::new(&mut self.studio.knobs.volume, 0.0..=150.0).step_by(1.0).show_value(false);
                    if ui.add(slider).on_hover_text(self.studio.knobs.text(volume)).changed() {
                        actions.push(Action::Studio(StudioAction::KnobChanged(volume)));
                    }
                    self.peak_meter(ui);
                    studio_ui::loudness_label(ui, &self.studio.loudness, &theme);
                    ui.separator();

                    let exporting = self.export.is_some();
                    let label = if exporting { "Export…" } else { "Exporter WAV" };
                    if ui.add_enabled(!exporting, Button::new(label))
                        .on_hover_text("Enregistre le morceau en .wav à côté du fichier (Ctrl+Maj+E)")
                        .clicked()
                    {
                        actions.push(Action::Export);
                    }
                    ui.separator();
                    let size_lbl = if self.ui_large { "A+" } else { "A" };
                    let size_btn = Button::new(RichText::new(size_lbl).size(14.0).strong()
                        .color(if self.ui_large { color(theme.background) } else { color(theme.foreground) }))
                        .fill(if self.ui_large { color(theme.accent) } else { color(theme.panel) })
                        .min_size(egui::vec2(40.0, 34.0));
                    if ui.add(size_btn).on_hover_text("Grande police (accessibilité) — Ctrl+Shift+A").clicked() {
                        actions.push(Action::ToggleUiSize);
                    }
                    if let Some(engine) = &self.engine {
                        let recording = engine.is_recording();
                        let rec = if recording {
                            RichText::new("● STOP REC").strong().color(color(theme.error))
                        } else {
                            RichText::new("● REC")
                        };
                        if ui.button(rec).on_hover_text("Enregistrer tout ce qui joue en WAV, changements live compris (F4)").clicked() {
                            actions.push(Action::Record);
                        }
                        if ui.selectable_label(self.metronome, "♩ Métronome").on_hover_text("Un clic sur chaque temps (Ctrl+T)").clicked() {
                            actions.push(Action::Metronome);
                        }
                    }
                    self.midi_menu(ui);
                    ui.separator();
                    if ui.selectable_label(self.stage_full, "Scène").on_hover_text("Visuels plein écran qui suivent la musique (F11 ou Ctrl+F, Échap pour revenir)").clicked() {
                        actions.push(Action::ToggleStage);
                    }
                    if ui.selectable_label(self.stage.director.vj, "VJ").on_hover_text("Mode VJ : la scène change toute seule toutes les 4 mesures").clicked() {
                        actions.push(Action::ToggleVj);
                    }
                    ui.separator();
                    if ui.selectable_label(self.studio.show_panel, "Studio").on_hover_text("Potards SWING/ECHO/SPACE/EQ, TAP, STEMS, LEARN, partage, mixeur, piano-roll, clips, démo (F6)").clicked() {
                        actions.push(Action::Studio(StudioAction::TogglePanel));
                    }
                });
            });
    }

    /// MIDI: clock out (a DAW follows LOOM) and clock in (LOOM follows a DAW).
    fn midi_menu(&mut self, ui: &mut egui::Ui) {
        let Some(engine) = &self.engine else { return };
        let active = self.midi_clock || self.midi_follow;
        let title = if active { RichText::new("MIDI ●").color(color(self.theme.accent)) } else { RichText::new("MIDI") };
        ui.menu_button(title, |ui| {
            ui.label(RichText::new(engine.midi_description()).color(color(self.theme.dim)));
            ui.separator();
            if ui.checkbox(&mut self.midi_clock, "Envoyer l'horloge MIDI")
                .on_hover_text("Un DAW ou une boîte à rythmes branché sur le port « LOOM » suit le tempo de LOOM")
                .changed()
            {
                engine.set_midi_clock(self.midi_clock);
            }
            if ui.checkbox(&mut self.midi_follow, "Suivre une horloge externe")
                .on_hover_text("LOOM prend le tempo, le départ et l'arrêt d'une horloge MIDI reçue")
                .changed()
            {
                engine.set_midi_follow(self.midi_follow);
            }
            ui.separator();
            ui.label(RichText::new("Un clavier MIDI joue en direct avec le son de la voix « lead ».\nLes voix « | midi N » sortent sur le port « LOOM ».").color(color(self.theme.dim)));
        });
    }

    /// The VIBE bar: describe the music, send, wait (spinner, cancel), undo.
    fn vibe_bar(&mut self, ui: &mut egui::Ui, actions: &mut Vec<Action>) {
        let theme = self.theme.clone();
        egui::Panel::top("vibe")
            .frame(egui::Frame::new().fill(color(theme.panel)).inner_margin(egui::Margin::symmetric(8, 6)))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(RichText::new("VIBE").size(16.0).strong().color(color(theme.accent)))
                        .on_hover_text("Décris la musique en mots : Claude écrit le code LOOM (Ctrl+G ou F2)");
                    let thinking = self.vibe.is_some();
                    let field = egui::TextEdit::singleline(&mut self.vibe_input)
                        .id(Id::new(VIBE_FIELD))
                        .hint_text("décris la musique, ex. « fais un groove afro plus sombre » · ?quinte pour le prof  (Ctrl+G)")
                        .desired_width((ui.available_width() - 330.0).max(160.0));
                    let response = ui.add_enabled(!thinking, field);
                    if response.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter)) {
                        actions.push(Action::Vibe);
                    }
                    if let Some(job) = &self.vibe {
                        ui.add(egui::Spinner::new().color(color(theme.accent)));
                        ui.label(format!("réfléchit… {} s", job.elapsed().as_secs()));
                        if ui.button("Annuler").on_hover_text("Arrêter d'attendre Claude (Échap)").clicked() {
                            actions.push(Action::CancelVibe);
                        }
                    } else {
                        if ui.add_enabled(!self.vibe_input.trim().is_empty(), Button::new("Envoyer"))
                            .on_hover_text("Claude réécrit le code selon ta demande (Entrée)")
                            .clicked()
                        {
                            actions.push(Action::Vibe);
                        }
                        if let Some(summary) = &self.vibe_summary {
                            ui.label(RichText::new(summary).monospace().color(color(theme.note)))
                                .on_hover_text("Voix changées par le dernier VIBE : + ajoutée, ~ modifiée, − retirée (↩ Annuler ou F3 pour revenir)");
                        }
                    }
                });
            });
    }

    /// "mesure 5 · drop 2/8" and "118 BPM".
    fn transport_text(&self) -> (String, String) {
        let Some(engine) = self.engine.as_ref().filter(|e| e.status().playing()) else {
            return ("arrêté".into(), format!("{:.0} BPM", self.program.bar_bpm(0)));
        };
        let status = engine.status();
        let song_bar = status.song_bar();
        let mut position = format!("mesure {}", status.bar() + 1);
        if let Some(section) = self.program.section_at(song_bar) {
            position.push_str(&format!(" · {section} ({}/{})", song_bar + 1, self.program.len_bars()));
        }
        (position, format!("{:.0} BPM", self.program.bar_bpm(song_bar)))
    }

    /// A small horizontal level meter for the output peak.
    fn peak_meter(&self, ui: &mut egui::Ui) {
        let peak = self.engine.as_ref().map_or(0.0, |e| if e.status().playing() { e.status().peak() } else { 0.0 });
        let voices = self.engine.as_ref().map_or(0, |e| e.status().voices());
        let (rect, response) = ui.allocate_exact_size(egui::vec2(90.0, 14.0), Sense::hover());
        let painter = ui.painter();
        painter.rect_filled(rect, 2, color(self.theme.background));
        let mut level = rect;
        level.set_width(rect.width() * peak.clamp(0.0, 1.0));
        let fill = if peak > 0.97 { color(self.theme.error) } else { color(self.theme.accent) };
        painter.rect_filled(level, 2, fill);
        response.on_hover_text(format!("Niveau de sortie {:.0} %, {voices} sons en cours", peak * 100.0));
    }

    /// Messages on the left, sound device and theme on the right.
    fn status_bar(&self, ui: &mut egui::Ui) {
        let dim = color(self.theme.dim);
        egui::Panel::bottom("status")
            .frame(egui::Frame::new().fill(color(self.theme.panel)).inner_margin(egui::Margin::symmetric(8, 4)))
            .show(ui, |ui| {
                // right side first, so the message gets whatever width is left
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    ui.label(RichText::new(format!("thème {}", self.theme.name)).color(dim));
                    ui.separator();
                    match (&self.engine, &self.audio_error) {
                        (Some(engine), _) => {
                            ui.label(RichText::new(engine.device_name()).color(dim))
                                .on_hover_text(format!("{} samples chargés · {}", engine.sample_count(), engine.midi_description()));
                        }
                        (None, error) => {
                            let label = ui.label(RichText::new("pas de son").color(color(self.theme.error)));
                            if let Some(e) = error {
                                label.on_hover_text(e);
                            }
                        }
                    }
                    ui.separator();
                    ui.with_layout(Layout::left_to_right(Align::Center), |ui| {
                        let message_color = if self.message.is_error { color(self.theme.error) } else { color(self.theme.foreground) };
                        let message = egui::Label::new(RichText::new(&self.message.text).color(message_color)).truncate();
                        ui.add(message).on_hover_text(&self.message.text);
                    });
                });
            });
    }

    /// The song library on the left.
    fn library_panel(&self, ui: &mut egui::Ui, actions: &mut Vec<Action>) {
        egui::Panel::left("library")
            .resizable(true)
            .default_size(210.0)
            .frame(egui::Frame::new().fill(color(self.theme.panel)).inner_margin(8))
            .show(ui, |ui| {
                ui.heading("Morceaux");
                ui.horizontal(|ui| {
                    if ui.button("Nouveau").on_hover_text("Nouveau morceau (Ctrl+N)").clicked() {
                        actions.push(Action::New);
                    }
                    if ui.button("⟳").on_hover_text("Actualiser la liste").clicked() {
                        actions.push(Action::RefreshLibrary);
                    }
                });
                ui.separator();
                egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                    if self.library.is_empty() {
                        ui.label(RichText::new(format!(
                            "Aucun fichier .loom trouvé.\nCtrl+S enregistre dans {}",
                            Library::user_dir().display()
                        )).color(color(self.theme.dim)));
                    }
                    for (label, files) in &self.library {
                        egui::CollapsingHeader::new(label).default_open(true).show(ui, |ui| {
                            for file in files {
                                let name = file.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
                                let is_current = self.path.as_deref() == Some(file.as_path())
                                    || self.path.as_ref().zip(file.canonicalize().ok()).is_some_and(|(a, b)| *a == b);
                                if ui.selectable_label(is_current, name).on_hover_text(file.display().to_string()).clicked() && !is_current {
                                    actions.push(Action::Open(file.clone()));
                                }
                            }
                        });
                    }
                });
            });
    }

    /// The live step grid under the editor, with a small live stage on its right.
    fn grid_panel(&mut self, ui: &mut egui::Ui, actions: &mut Vec<Action>) {
        let playing = self.is_playing();
        let (bar, phase) = match &self.engine {
            Some(engine) if playing => (engine.status().song_bar(), Some(engine.status().phase())),
            _ => (0, None),
        };
        let rows = loom_app::grid(&self.program, bar, grid::COLUMNS);
        egui::Panel::bottom("grid")
            .resizable(true)
            .default_size(grid::height_for(rows.len()) + 44.0)
            .frame(egui::Frame::new().fill(color(self.theme.panel)).inner_margin(8))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(RichText::new("Grille").strong());
                    let hint = if playing { format!("mesure {} du morceau", bar + 1) } else { "aperçu (arrêté)".into() };
                    ui.label(RichText::new(hint).color(color(self.theme.dim)));
                    ui.label(RichText::new("· clic sur un nom = mute").color(color(self.theme.dim)));
                });
                // the thumbnail only when there is room for the grid next to it
                let thumb_height = ui.available_height().clamp(60.0, 150.0);
                let thumb_width = thumb_height * 16.0 / 9.0;
                let with_thumbnail = ui.available_width() > thumb_width + 520.0;
                ui.horizontal_top(|ui| {
                    let grid_width = if with_thumbnail { ui.available_width() - thumb_width - 10.0 } else { ui.available_width() };
                    ui.allocate_ui(egui::vec2(grid_width, ui.available_height()), |ui| {
                        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                            if let Some(voice) = grid::show(ui, &rows, phase, &self.muted, &self.theme) {
                                actions.push(Action::ToggleMute(voice));
                            }
                        });
                    });
                    if with_thumbnail {
                        let (rect, response) = ui.allocate_exact_size(egui::vec2(thumb_width, thumb_height), Sense::click());
                        self.stage.paint(ui.painter(), rect, &Look::from_theme(&self.theme), None);
                        let response = response.on_hover_text(format!(
                            "Scène « {} » : clic ou F11 pour le plein écran (scene: dans le code pour la choisir)",
                            self.stage.scene().name()
                        ));
                        if response.hovered() {
                            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                        }
                        if response.clicked() {
                            actions.push(Action::ToggleStage);
                        }
                    }
                });
            });
    }

    /// The full-window SCÈNE, with a few buttons in the bottom right corner.
    fn stage_view(&mut self, ui: &mut egui::Ui, actions: &mut Vec<Action>) {
        let look = Look::from_theme(&self.theme);
        let status = self.engine.as_ref().map(|e| e.status()).filter(|s| s.playing());
        let song_bar = status.map_or(0, |s| s.song_bar());
        let section = status.and_then(|_| self.program.section_at(song_bar)).map(str::to_string);
        let file_title = self.path.as_ref().and_then(|p| p.file_stem()).map(|n| n.to_string_lossy().into_owned());
        let title = self.stage.director.title.clone().or(file_title).unwrap_or_default();
        let overlay = Overlay { title: &title, section: section.as_deref(), bar: status.map(|s| s.bar() + 1), pure: !self.stage_text };

        egui::CentralPanel::default().frame(egui::Frame::NONE.fill(look.bg)).show(ui, |ui| {
            let rect = ui.max_rect();
            self.stage.paint(ui.painter(), rect, &look, Some(&overlay));

            let size = egui::vec2(120.0, 28.0);
            let mut corner = rect.right_bottom() - egui::vec2(16.0 + size.x, 60.0 + size.y);
            let mut button = |ui: &mut egui::Ui, label: RichText, hover: &str, action: Action| {
                let button_rect = egui::Rect::from_min_size(corner, size);
                if ui.put(button_rect, Button::new(label).selected(action == Action::ToggleVj && self.stage.director.vj)).on_hover_text(hover).clicked() {
                    actions.push(action);
                }
                corner.x -= size.x + 8.0;
            };
            button(ui, RichText::new("Retour au code"), "Revenir à l'éditeur (Échap ou F11)", Action::ToggleStage);
            button(ui, RichText::new("Scène suivante"), "Changer de scène (N)", Action::NextScene);
            button(ui, RichText::new("VJ"), "La scène change toute seule toutes les 4 mesures (V)", Action::ToggleVj);
        });
    }

    /// The code editor, filling the rest of the window.
    fn editor(&mut self, ui: &mut egui::Ui) {
        let palette = style::palette(&self.theme, self.ui_large);
        let editor_id = Id::new(EDITOR_ID);
        let playing = self.is_playing();
        let background = color(self.theme.background);
        let highlighter = &mut self.highlighter;
        let text = &mut self.text;

        let changed = egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(background).inner_margin(10))
            .show(ui, |ui| {
                egui::ScrollArea::both().auto_shrink([false, false]).show(ui, |ui| {
                    let mut layouter = |ui: &egui::Ui, buf: &dyn egui::TextBuffer, _wrap_width: f32| {
                        highlighter.galley(ui, buf.as_str(), &palette)
                    };
                    let edit = egui::TextEdit::multiline(text)
                        .id(editor_id)
                        .code_editor()
                        .frame(egui::Frame::NONE)
                        .desired_width(f32::INFINITY)
                        .min_size(ui.available_size())
                        .layouter(&mut layouter);
                    ui.add(edit).changed()
                })
                .inner
            })
            .inner;

        if self.first_frame {
            ui.memory_mut(|m| m.request_focus(editor_id));
            self.first_frame = false;
        }
        // while stopped, the grid previews what you type
        if changed && !playing {
            self.set_program(Program::parse(&self.text));
        }
    }

    /// "Unsaved changes" dialog, shown when an action would replace the text.
    fn confirm_dialog(&mut self, ctx: &egui::Context, actions: &mut Vec<Action>) {
        let Some(pending) = self.confirm.clone() else { return };

        #[derive(PartialEq)]
        enum Choice { None, SaveFirst, Discard, Cancel }
        let mut choice = Choice::None;

        let response = egui::Modal::new(Id::new("confirm-discard")).show(ctx, |ui| {
            ui.set_max_width(380.0);
            ui.heading("Modifications non enregistrées");
            ui.label(format!("« {} » a été modifié. Que faire avant de continuer ?", self.file_name()));
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                if ui.button("Enregistrer").clicked() { choice = Choice::SaveFirst; }
                if ui.button("Ne pas enregistrer").clicked() { choice = Choice::Discard; }
                if ui.button("Annuler").clicked() { choice = Choice::Cancel; }
            });
        });
        if response.should_close() && choice == Choice::None {
            choice = Choice::Cancel;
        }

        match choice {
            Choice::None => {}
            Choice::Cancel => self.confirm = None,
            Choice::Discard => {
                self.confirm = None;
                self.perform_now(pending, ctx);
            }
            Choice::SaveFirst => {
                self.confirm = None;
                if self.save() {
                    actions.push(pending);
                }
            }
        }
    }
}

impl eframe::App for LoomApp {
    /// Runs every frame, even when the window is hidden: theme, export, repaints.
    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if let Some(theme) = self.theme_watcher.poll() {
            style::apply(ctx, &theme);
            self.say(format!("Thème : {}", theme.name));
            self.theme = theme;
        }
        self.poll_export();
        self.poll_vibe();
        self.studio_frame(ctx);
        self.teacher_frame(ctx);
        if self.stage_full || self.is_playing() {
            self.stage.update(self.engine.as_ref().map(|e| e.status()), &self.program);
        }
        if self.stage_full {
            ctx.request_repaint(); // the stage moves even in silence
        }
        if self.vibe.is_some() {
            ctx.request_repaint_after(Duration::from_millis(100)); // spinner, and pick up the answer
        }
        if self.is_playing() {
            ctx.request_repaint(); // keep the playhead moving
        } else {
            // wake up now and then to notice a theme change
            ctx.request_repaint_after(Duration::from_millis(500));
        }
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let mut actions = self.shortcuts(ui.ctx());
        if self.stage_full {
            self.stage_view(ui, &mut actions);
        } else {
            self.top_bar(ui, &mut actions);
            self.vibe_bar(ui, &mut actions);
            self.teacher_bar(ui, &mut actions);
            self.studio_bar(ui, &mut actions);
            self.status_bar(ui);
            self.library_panel(ui, &mut actions);
            self.grid_panel(ui, &mut actions);
            self.studio_panels(ui, &mut actions);
            self.teacher_panels(ui, &mut actions);
            self.editor(ui);
        }
        let ctx = ui.ctx().clone();
        self.teacher_windows(&ctx, &mut actions);
        self.import_dialog(&ctx, &mut actions);
        self.confirm_dialog(&ctx, &mut actions);
        for action in actions {
            self.perform(action, &ctx);
        }
    }
}
