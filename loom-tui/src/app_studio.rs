//! The STUDIO in the terminal: what its keys do, and the work done every frame.
//!
//! | key      | what                                                         |
//! |----------|--------------------------------------------------------------|
//! | Alt+M    | MIX view: voices (fader, pan, M, S, clip) then master knobs  |
//! | Alt+A    | TIMELINE: edit the `song:` arrangement                        |
//! | Alt+R    | ROLL: piano roll instead of the grid                          |
//! | Alt+T    | TAP tempo                                                     |
//! | Alt+S    | STEMS: one WAV per voice in `<song>-stems/`                   |
//! | Alt+P    | share: `LOOM1:` code in the status line and `<song>.share.txt`|
//! | Alt+I    | import a code, a link or LOOM code                            |
//! | Alt+D    | DÉMO: tour of the example songs (any key stops it)           |
//!
//! This file is a child module of `app.rs`, so it can use the app's state directly.

use std::path::{Path, PathBuf};
use std::time::Instant;

use loom_app::Library;
use loom_audio::SCOPE_LEN;
use loom_core::Program;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use super::{App, Mode, Pending};
use crate::editor::Editor;
use loom_app::studio::clips::{Clips, SLOTS};
use loom_app::studio::demo::Demo;
use loom_app::studio::knobs::{Knob, Knobs};
use loom_app::studio::learn::Learn;
use loom_app::studio::loudness::Loudness;
use loom_app::studio::mixer::{Mixer, Param};
use loom_app::studio::songline::{self, SongEdit};
use loom_app::studio::stems::StemsJob;
use loom_app::studio::tap::TapTempo;
use loom_app::studio::{share, demo};

/// The demo tour, and the song to bring back when it stops.
pub struct DemoRun {
    pub demo: Demo,
    saved_text: String,
    saved_dirty: bool,
    saved_path: Option<PathBuf>,
    was_vj: bool,
}

/// Everything the studio remembers.
pub struct Studio {
    pub knobs: Knobs,
    pub mixer: Mixer,
    pub loudness: Loudness,
    pub learn: Learn,
    pub stems: Option<StemsJob>,
    pub clips: Clips,
    pub demo: Option<DemoRun>,
    /// The side panel shows the piano roll instead of the grid.
    pub show_roll: bool,
    /// Alt+F: la partition (grande portée) à la place de la grille.
    pub show_notation: bool,
    tap: TapTempo,
    scope: Box<[f32; SCOPE_LEN]>,
    clock: Instant,
    last_frame: Instant,
}

impl Studio {
    pub fn new(sample_rate: u32, volume: f32) -> Studio {
        let mut knobs = Knobs::default();
        knobs.set(Knob::Volume, volume * 100.0);
        Studio {
            knobs,
            mixer: Mixer::default(),
            loudness: Loudness::new(sample_rate),
            learn: Learn::load(),
            stems: None,
            clips: Clips::default(),
            demo: None,
            show_roll: false,
            show_notation: false,
            tap: TapTempo::default(),
            scope: Box::new([0.0; SCOPE_LEN]),
            clock: Instant::now(),
            last_frame: Instant::now(),
        }
    }
}

/// A row of the MIX view: a voice strip, or a master knob.
pub enum MixRow {
    Voice(usize),
    Knob(Knob),
}

impl App {
    /// The code the engine plays: the editor's, with any CLIPS variations.
    pub(super) fn engine_code(&self, text: &str) -> String {
        self.studio.clips.apply(text)
    }

    /// The program the studio views show: what plays, else the editor's code.
    pub fn studio_program(&self) -> Program {
        if self.running_text.is_some() { self.program.clone() } else { Program::parse(&self.editor.text()) }
    }

    /// Row `row` of the MIX view.
    pub fn mix_row(&self, row: usize) -> MixRow {
        let voices = self.studio.mixer.visible().len();
        if row < voices { MixRow::Voice(row) } else { MixRow::Knob(Knob::ALL[(row - voices).min(Knob::ALL.len() - 1)]) }
    }

    /// Work done every frame: meters, loudness, MIDI LEARN, stems, demo.
    pub(super) fn studio_tick(&mut self) {
        let dt = self.studio.last_frame.elapsed().as_secs_f32();
        self.studio.last_frame = Instant::now();
        let now = self.studio.clock.elapsed().as_secs_f64();
        let mut learned = None;
        if let Ok(engine) = &self.engine {
            let status = engine.status();
            if status.playing() {
                status.scope(&mut self.studio.scope[..]);
                self.studio.loudness.feed(&self.studio.scope[..], status.peak(), now);
                self.studio.mixer.update_meters(&status.voice_levels(), dt);
            } else {
                self.studio.loudness.reset();
                self.studio.mixer.update_meters(&[], dt);
            }
            let controls = status.take_controls();
            if !controls.is_empty() {
                let events = self.studio.learn.handle(&controls, &mut self.studio.knobs);
                for knob in &events.changed {
                    self.studio.knobs.apply(*knob, engine);
                }
                learned = Some(events);
            }
        }
        if let Some(events) = learned {
            if events.changed.contains(&Knob::Volume) { self.volume = self.studio.knobs.volume / 100.0; }
            if let Some((cc, knob)) = events.bound { self.info(format!("LEARN : CC{cc} → {} (enregistré)", knob.label())); }
            if let Some(e) = events.error { self.error(e); }
        }
        if let Some(result) = self.studio.stems.as_mut().and_then(StemsJob::poll) {
            self.studio.stems = None;
            match result {
                Ok((dir, n)) => self.info(format!("STEMS : {n} pistes → {}", dir.display())),
                Err(e) => self.error(e),
            }
        }
        if let Some(next) = self.studio.demo.as_mut().and_then(|d| d.demo.next_if_due().map(Path::to_path_buf)) {
            self.demo_play(&next);
        }
    }

    /// Keys for the studio. Returns true when the key was used here.
    pub(super) fn studio_key(&mut self, key: KeyEvent) -> bool {
        if self.studio.demo.is_some() {
            self.stop_demo(); // any key stops the demo
            return true;
        }
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        let alt = key.modifiers.contains(KeyModifiers::ALT);
        match &self.mode {
            Mode::Mixer { .. } => { self.mixer_key(key, ctrl, alt); true }
            Mode::Timeline { .. } => { self.timeline_key(key, alt); true }
            Mode::Import { .. } => { self.import_key(key, ctrl); true }
            Mode::Edit if alt => self.studio_shortcut(key.code),
            _ => false,
        }
    }

    /// Alt+letter in the editor.
    fn studio_shortcut(&mut self, code: KeyCode) -> bool {
        match code {
            KeyCode::Char('m' | 'M') => self.open_mixer(),
            KeyCode::Char('a' | 'A') => self.open_timeline(),
            KeyCode::Char('r' | 'R') => {
                self.studio.show_roll = !self.studio.show_roll;
                self.info(if self.studio.show_roll { "piano-roll (Alt+R pour revenir à la grille)" } else { "grille" });
            }
            KeyCode::Char('t' | 'T') => self.tap_tempo(),
            KeyCode::Char('s' | 'S') => self.start_stems(),
            KeyCode::Char('p' | 'P') => self.export_share(),
            KeyCode::Char('i' | 'I') => self.mode = Mode::Import { input: String::new() },
            KeyCode::Char('d' | 'D') => self.start_demo(),
            KeyCode::Char('x' | 'X') => self.export_midi(),
            KeyCode::Char('n' | 'N') => self.other_song(false),
            KeyCode::Char('z' | 'Z') => self.other_song(true),
            KeyCode::Char('k' | 'K') => self.toggle_lien(),
            KeyCode::Char('c' | 'C') => self.next_theme(),
            KeyCode::Char('f' | 'F') => {
                self.studio.show_notation = !self.studio.show_notation;
                self.info(if self.studio.show_notation { "partition (Alt+F pour revenir)" } else { "grille" });
            }
            _ => return false,
        }
        true
    }

    // ---- MIX -------------------------------------------------------------

    fn open_mixer(&mut self) {
        let program = self.studio_program();
        self.studio.mixer.sync(&program);
        self.mode = Mode::Mixer { row: 0, param: Param::Gain };
        self.info("MIX : ↑↓ choisir · ←→ volume/pan · +/- régler · m mute · s solo · c clip · l LEARN · Échap fermer");
    }

    fn mixer_key(&mut self, key: KeyEvent, ctrl: bool, alt: bool) {
        let Mode::Mixer { row, param } = &mut self.mode else { return };
        let rows = self.studio.mixer.visible().len() + Knob::ALL.len();
        let (mut r, mut p) = (*row, *param);
        let mut steps = 0.0;
        match key.code {
            KeyCode::Esc | KeyCode::Enter => { self.mode = Mode::Edit; return; }
            KeyCode::Char('m' | 'M') if alt => { self.mode = Mode::Edit; return; }
            KeyCode::Char('r') if ctrl => { self.run(); return; }
            KeyCode::F(5) => { self.run(); return; }
            KeyCode::F(6) => { self.stop(); return; }
            KeyCode::Up => r = r.saturating_sub(1),
            KeyCode::Down => r = (r + 1).min(rows - 1),
            KeyCode::Left => p = Param::Gain,
            KeyCode::Right => p = Param::Pan,
            KeyCode::Char('+' | '=') => steps = 1.0,
            KeyCode::Char('-' | '_') => steps = -1.0,
            KeyCode::PageUp => steps = 5.0,
            KeyCode::PageDown => steps = -5.0,
            _ => {}
        }
        if let Mode::Mixer { row, param } = &mut self.mode { (*row, *param) = (r, p); }
        let engine = self.engine.as_ref().ok();
        match (self.mix_row(r), key.code) {
            (MixRow::Voice(i), KeyCode::Char('m' | 'M')) if !alt => {
                let name = self.studio.mixer.strips[i].name.clone();
                self.toggle_mute_named(&name);
            }
            (MixRow::Voice(i), KeyCode::Char('s' | 'S')) => {
                self.studio.mixer.toggle_solo(i, engine);
                let s = &self.studio.mixer.strips[i];
                self.info(format!("{} {}", s.name, if s.solo { "en solo" } else { "solo retiré" }));
            }
            (MixRow::Voice(i), KeyCode::Char('c' | 'C')) => self.next_clip(i),
            (MixRow::Voice(i), _) if steps != 0.0 => self.studio.mixer.nudge(i, p, steps, engine),
            (MixRow::Knob(knob), KeyCode::Char('l' | 'L')) => {
                if !self.studio.learn.on { self.studio.learn.toggle(); }
                self.studio.learn.arm(knob);
                self.info(format!("LEARN : bouge un potard MIDI pour lier « {} »", knob.label()));
            }
            (MixRow::Knob(knob), _) if steps != 0.0 => {
                self.studio.knobs.nudge(knob, steps);
                if let Some(e) = engine { self.studio.knobs.apply(knob, e); }
                if knob == Knob::Volume { self.volume = self.studio.knobs.volume / 100.0; }
            }
            _ => {}
        }
    }

    /// Mute or unmute a voice by name (the grid's Alt+N does it by number).
    fn toggle_mute_named(&mut self, name: &str) {
        let muted = !self.muted.iter().any(|m| m == name);
        if muted { self.muted.push(name.to_string()) } else { self.muted.retain(|m| m != name) }
        if let Ok(e) = &self.engine { e.set_muted(name, muted); }
        self.info(format!("{name} {}", if muted { "coupé" } else { "rétabli" }));
    }

    /// `c` in the mixer: the voice's next CLIPS slot, heard at the next bar.
    fn next_clip(&mut self, i: usize) {
        let name = self.studio.mixer.strips[i].name.clone();
        let current = self.studio.clips.verb_for(&name);
        let index = SLOTS.iter().position(|(_, v)| *v == current).unwrap_or(0);
        let (label, verb) = SLOTS[(index + 1) % SLOTS.len()];
        self.studio.clips.set(&name, verb);
        if self.is_playing() { self.run(); }
        self.info(format!("clip {name} : {label}{}", if self.is_playing() { " (à la prochaine mesure)" } else { "" }));
    }

    // ---- TIMELINE --------------------------------------------------------

    fn open_timeline(&mut self) {
        if songline::sections(&self.editor.text()).is_empty() {
            self.error("TIMELINE : pas d'arrangement — écris par exemple  song: intro*4 couplet*8 refrain*8");
            return;
        }
        self.mode = Mode::Timeline { selected: 0 };
        self.info("TIMELINE : ←→ section · +/- mesures · d dupliquer · x supprimer · < > déplacer · Échap fermer");
    }

    fn timeline_key(&mut self, key: KeyEvent, alt: bool) {
        let Mode::Timeline { selected } = self.mode else { return };
        let song = songline::sections(&self.editor.text());
        let shift = key.modifiers.contains(KeyModifiers::SHIFT);
        let edit = match key.code {
            KeyCode::Esc | KeyCode::Enter => { self.mode = Mode::Edit; return; }
            KeyCode::Char('a' | 'A') if alt => { self.mode = Mode::Edit; return; }
            KeyCode::Left if shift => SongEdit::MoveLeft,
            KeyCode::Right if shift => SongEdit::MoveRight,
            KeyCode::Left => { self.mode = Mode::Timeline { selected: selected.saturating_sub(1) }; return; }
            KeyCode::Right => { self.mode = Mode::Timeline { selected: (selected + 1).min(song.len().saturating_sub(1)) }; return; }
            KeyCode::Char('+' | '=') => SongEdit::Longer,
            KeyCode::Char('-' | '_') => SongEdit::Shorter,
            KeyCode::Char('d' | 'D') => SongEdit::Duplicate,
            KeyCode::Char('x' | 'X') | KeyCode::Delete | KeyCode::Backspace => SongEdit::Delete,
            KeyCode::Char('<' | ',') => SongEdit::MoveLeft,
            KeyCode::Char('>' | '.') => SongEdit::MoveRight,
            _ => return,
        };
        let (song, new_selected) = songline::edit_sections(&song, selected, edit);
        self.editor.replace_text(&songline::set_song(&self.editor.text(), &song));
        if self.is_playing() { self.run(); }
        let total: u32 = song.iter().map(|(_, b)| b).sum();
        self.info(format!("arrangement : {} sections · {total} mesures", song.len()));
        self.mode = match new_selected { Some(s) => Mode::Timeline { selected: s }, None => Mode::Edit };
    }

    // ---- TAP, STEMS, share ----------------------------------------------

    fn tap_tempo(&mut self) {
        match self.studio.tap.tap() {
            Some(bpm) => {
                self.editor.replace_text(&songline::set_bpm(&self.editor.text(), bpm));
                if self.is_playing() { self.run(); }
                self.info(format!("TAP : {bpm} bpm (ligne bpm: réécrite) — Alt+T encore pour affiner"));
            }
            None => self.info("TAP : continue de taper Alt+T en rythme…"),
        }
    }

    fn start_stems(&mut self) {
        if self.studio.stems.is_some() { return; }
        let sample_rate = self.engine.as_ref().map_or(48_000, |e| e.sample_rate());
        match StemsJob::start(self.editor.text(), self.path.as_deref(), sample_rate, || {}) {
            Some(job) => {
                self.info(format!("STEMS : {} voix → {}…", job.total, job.dir.display()));
                self.studio.stems = Some(job);
            }
            None => self.error("STEMS : aucune voix à exporter"),
        }
    }

    /// Alt+P: the `LOOM1:` code goes into a file next to the song and the status line.
    fn export_share(&mut self) {
        let code = share::export(&self.editor.text(), &self.studio.knobs);
        let path = share::share_path(self.path.as_deref());
        let written = path.parent().map_or(Ok(()), std::fs::create_dir_all).and_then(|_| std::fs::write(&path, format!("{code}\n")));
        match written {
            Ok(()) => self.info(format!("partage → {} · {code}", path.display())),
            Err(e) => self.error(format!("partage : impossible d'écrire {} : {e} · {code}", path.display())),
        }
    }

    fn import_key(&mut self, key: KeyEvent, ctrl: bool) {
        let Mode::Import { input } = &mut self.mode else { return };
        match key.code {
            KeyCode::Enter => {
                let pasted = std::mem::take(input);
                self.mode = Mode::Edit;
                if share::import(&pasted, &self.studio.knobs).is_none() {
                    self.error("import : ni un code LOOM1:, ni un lien LOOM, ni du code LOOM");
                } else {
                    self.guarded(Pending::Import(pasted));
                }
            }
            KeyCode::Esc => self.mode = Mode::Edit,
            KeyCode::Backspace => { input.pop(); }
            KeyCode::Char('u') if ctrl => input.clear(),
            KeyCode::Char(c) if !ctrl => input.push(c),
            _ => {}
        }
    }

    /// Load an imported song (a new, unsaved song) and its knobs.
    pub(super) fn import(&mut self, pasted: &str) {
        let Some(imported) = share::import(pasted, &self.studio.knobs) else { return };
        self.forget_song_settings();
        self.editor = Editor::new(&imported.code);
        self.editor.dirty = true;
        self.path = None;
        self.stage.load(&imported.code, &Program::parse(&imported.code));
        self.studio.knobs = imported.knobs;
        self.volume = self.studio.knobs.volume / 100.0;
        if let Ok(e) = &self.engine { self.studio.knobs.apply_all(e); }
        let knobs = if imported.has_knobs { " avec ses potards" } else { "" };
        self.info(format!("morceau importé{knobs} — Ctrl+R pour l'entendre, Ctrl+S pour l'enregistrer"));
    }

    /// Mixer settings and clips belong to the song that was open.
    pub(super) fn forget_song_settings(&mut self) {
        self.studio.mixer.reset(self.engine.as_ref().ok());
        self.studio.clips.clear();
    }

    // ---- DÉMO ------------------------------------------------------------

    fn start_demo(&mut self) {
        let Some(tour) = Demo::start(demo::playlist(&Library::scan())) else {
            self.error("DÉMO : aucun morceau d'exemple trouvé (lance ./install.sh)");
            return;
        };
        let first = tour.current().to_path_buf();
        self.studio.demo = Some(DemoRun {
            demo: tour,
            saved_text: self.editor.text(),
            saved_dirty: self.editor.dirty,
            saved_path: self.path.clone(),
            was_vj: self.stage.director.vj,
        });
        self.stage.director.vj = true;
        self.mode = Mode::Stage;
        self.demo_play(&first);
    }

    /// Play one song of the tour (without changing the "last opened" song).
    fn demo_play(&mut self, path: &Path) {
        match std::fs::read_to_string(path) {
            Ok(text) => {
                self.forget_song_settings();
                self.editor = Editor::new(&text);
                self.path = Some(path.to_path_buf());
                self.run();
                if let Some(run) = &self.studio.demo { self.info(format!("{} — une touche pour arrêter", run.demo.text())); }
            }
            Err(e) => self.error(format!("DÉMO : impossible de lire {} : {e}", path.display())),
        }
    }

    /// Stop the tour and bring back the user's song as it was.
    pub(super) fn stop_demo(&mut self) {
        let Some(run) = self.studio.demo.take() else { return };
        self.stop();
        self.forget_song_settings();
        self.editor = Editor::new(&run.saved_text);
        self.editor.dirty = run.saved_dirty;
        self.path = run.saved_path;
        self.stage.director.vj = run.was_vj;
        self.running_text = None;
        self.program = Program::parse(&run.saved_text);
        self.stage.load(&run.saved_text, &self.program);
        self.mode = Mode::Edit;
        self.info("démo arrêtée — ton morceau est revenu");
    }
}
