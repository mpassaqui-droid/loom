//! The LOOM window: its state, and what happens each frame.
//!
//! Each frame the UI is drawn top to bottom (transport bar, status bar,
//! library, grid, editor). Drawing never changes the song directly: buttons
//! and keyboard shortcuts produce [`Action`]s, which [`LoomApp::perform`]
//! carries out once the frame is drawn. That keeps "what the user asked for"
//! in one place.

use std::path::{Path, PathBuf};
use std::sync::mpsc::Receiver;
use std::time::Duration;

use eframe::egui::{self, Align, Button, Id, Key, Layout, Modifiers, RichText, Sense};
use loom_app::{Library, Theme, ThemeWatcher, WELCOME};
use loom_audio::Engine;
use loom_core::Program;

use crate::editor::Highlighter;
use crate::export::{self, ExportResult};
use crate::grid;
use crate::style::{self, color};

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
    /// Close the window.
    Quit,
}

impl Action {
    /// Would this throw away the editor text (and its unsaved edits)?
    fn loses_edits(&self) -> bool {
        matches!(self, Action::New | Action::Open(_) | Action::Quit)
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
    volume: f32,
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
    first_frame: bool,
    /// Set once the user agreed to close, so the next close request goes through.
    closing: bool,
}

impl LoomApp {
    /// Open the sound card, load the theme and the first song.
    /// `file` is the command-line argument; without it, the last song reopens.
    pub fn new(cc: &eframe::CreationContext<'_>, file: Option<PathBuf>) -> LoomApp {
        let theme = Theme::load();
        style::apply_text_sizes(&cc.egui_ctx);
        style::apply(&cc.egui_ctx, &theme);

        let (engine, audio_error) = match Engine::start() {
            Ok(engine) => (Some(engine), None),
            Err(e) => (None, Some(e)),
        };
        let volume = 0.8;
        if let Some(engine) = &engine {
            engine.set_volume(volume);
        }

        let mut app = LoomApp {
            engine,
            audio_error,
            volume,
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
            first_frame: true,
            closing: false,
        };
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
                let text = format!("Son prêt : {} ({} Hz). Ctrl+Entrée pour jouer.", engine.device_name(), engine.sample_rate());
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
            Action::Quit => {
                self.closing = true;
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
        }
    }

    /// RUN: send the code to the engine. Starts playing if stopped; while
    /// playing, the change lands on the next bar line.
    fn run(&mut self) {
        let program = Program::parse(&self.text);
        let voice_count = program.voices.len();
        let Some(engine) = &self.engine else {
            self.program = program;
            self.warn("Pas de sortie audio : impossible de jouer (l'export WAV reste possible).");
            return;
        };
        let was_playing = engine.status().playing();
        engine.run(&self.text);
        self.program = program;
        if voice_count == 0 {
            self.warn("Aucune voix reconnue. Une voix s'écrit par exemple  kick: x . x .");
        } else if was_playing {
            self.say(format!("Code envoyé ({voice_count} voix) : il démarre à la prochaine mesure."));
        } else {
            self.say(format!("Lecture ({voice_count} voix)."));
        }
    }

    fn stop(&mut self) {
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
    fn set_text(&mut self, text: String, path: Option<PathBuf>) {
        self.saved_text = text.clone();
        self.text = text;
        self.path = path;
        if !self.is_playing() {
            self.program = Program::parse(&self.text);
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
        ctx.input_mut(|i| {
            let mut actions = Vec::new();
            // most specific first: Ctrl+Shift+E before any plain Ctrl shortcut
            if i.consume_key(Modifiers::COMMAND | Modifiers::SHIFT, Key::E) {
                actions.push(Action::Export);
            }
            if i.consume_key(Modifiers::COMMAND, Key::Enter) {
                actions.push(Action::Run);
            }
            if i.consume_key(Modifiers::COMMAND, Key::Period) || i.consume_key(Modifiers::NONE, Key::Escape) {
                actions.push(Action::Stop);
            }
            if i.consume_key(Modifiers::COMMAND, Key::S) {
                actions.push(Action::Save);
            }
            if i.consume_key(Modifiers::COMMAND, Key::N) {
                actions.push(Action::New);
            }
            actions
        })
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
                    let slider = egui::Slider::new(&mut self.volume, 0.0..=1.5).show_value(false);
                    if ui.add(slider).changed() {
                        if let Some(engine) = &self.engine {
                            engine.set_volume(self.volume);
                        }
                    }
                    self.peak_meter(ui);
                    ui.separator();

                    let exporting = self.export.is_some();
                    let label = if exporting { "Export…" } else { "Exporter WAV" };
                    if ui.add_enabled(!exporting, Button::new(label))
                        .on_hover_text("Enregistre le morceau en .wav à côté du fichier (Ctrl+Maj+E)")
                        .clicked()
                    {
                        actions.push(Action::Export);
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
                            ui.label(RichText::new(engine.device_name()).color(dim));
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

    /// The live step grid under the editor.
    fn grid_panel(&self, ui: &mut egui::Ui, actions: &mut Vec<Action>) {
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
                egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                    if let Some(voice) = grid::show(ui, &rows, phase, &self.muted, &self.theme) {
                        actions.push(Action::ToggleMute(voice));
                    }
                });
            });
    }

    /// The code editor, filling the rest of the window.
    fn editor(&mut self, ui: &mut egui::Ui) {
        let palette = style::palette(&self.theme);
        let editor_id = Id::new("loom-editor");
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
            self.program = Program::parse(&self.text);
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
        if self.is_playing() {
            ctx.request_repaint(); // keep the playhead moving
        } else {
            // wake up now and then to notice a theme change
            ctx.request_repaint_after(Duration::from_millis(500));
        }
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let mut actions = self.shortcuts(ui.ctx());
        self.top_bar(ui, &mut actions);
        self.status_bar(ui);
        self.library_panel(ui, &mut actions);
        self.grid_panel(ui, &mut actions);
        self.editor(ui);
        let ctx = ui.ctx().clone();
        self.confirm_dialog(&ctx, &mut actions);
        for action in actions {
            self.perform(action, &ctx);
        }
    }
}
