//! What the STUDIO does inside the window: carrying out [`StudioAction`]s,
//! the work done every frame (meters, loudness, MIDI LEARN, stems, demo) and
//! drawing the studio panels.
//!
//! This file is a child module of `app.rs`, so it can read and change the
//! app's state directly; `app.rs` only calls the few functions below.

use std::path::Path;

use eframe::egui;
use loom_app::Library;
use loom_core::Program;

use super::{Action, LoomApp};
use loom_app::studio::{demo, learn::LearnEvents, roll, sampletool::SampleSpec, share, songline, stems::StemsJob};
use crate::studio_ui::{self, DemoRun, StudioAction};

impl LoomApp {
    /// The code the engine plays: the editor's, with any CLIPS variations.
    pub(super) fn engine_code(&self) -> String {
        self.studio.clips.apply(&self.text)
    }

    /// Carry out a studio action.
    pub(super) fn studio_action(&mut self, action: StudioAction, ctx: &egui::Context) {
        let engine = self.engine.as_ref();
        match action {
            StudioAction::TogglePanel => self.studio.show_panel = !self.studio.show_panel,
            StudioAction::ToggleMixer => self.studio.show_mixer = !self.studio.show_mixer,
            StudioAction::ToggleRoll => self.studio.show_roll = !self.studio.show_roll,
            StudioAction::ToggleClips => self.studio.show_clips = !self.studio.show_clips,
            StudioAction::ToggleNotation => self.studio.show_notation = !self.studio.show_notation,
            StudioAction::NotationTranspose(dt) => self.notation_transpose(dt),
            StudioAction::ArmNotation(voice) => {
                self.studio.notation_voice = Some(voice.clone());
                self.say(format!("PARTITION : « {voice} » armée — ◀ ▶ pour la transposer."));
            }
            StudioAction::ToggleSampler => self.studio.show_sampler = !self.studio.show_sampler,
            StudioAction::ArmSampler(voice) => {
                self.studio.sampler_voice = Some(voice.clone());
                if let Some(v) = self.program.voices.iter().find(|x| x.name == voice) {
                    self.studio.sampler_spec = SampleSpec::from_synth(&v.synth);
                }
                self.say(format!("SAMPLER : « {voice} » armée — choisis un échantillon puis APPLIQUER."));
            }
            StudioAction::SamplerApply => self.sampler_apply(),
            StudioAction::SamplerDir(dir) => self.studio.sampler_dir = std::path::PathBuf::from(dir),
            StudioAction::SamplerLoadWav(path) => self.sampler_load_wav(&path),
            StudioAction::SamplerReloadBank => {
                loom_audio::reload_bank();
                self.studio.sampler_msg = Some("Banque re-scannée depuis le disque (tes fichiers de session sont conservés).".into());
            }
            StudioAction::KnobChanged(knob) => {
                if let Some(engine) = engine {
                    self.studio.knobs.apply(knob, engine);
                }
            }
            StudioAction::StripChanged(i) => {
                if let Some(strip) = self.studio.mixer.strips.get(i).cloned() {
                    self.studio.mixer.set_gain(i, strip.gain, engine);
                    self.studio.mixer.set_pan(i, strip.pan, engine);
                }
            }
            StudioAction::ToggleSolo(i) => self.studio.mixer.toggle_solo(i, engine),
            StudioAction::SendLevel(i) => {
                if let Some(send) = self.studio.mixer.sends.get(i).cloned() {
                    self.studio.mixer.set_send_level(i, send.level, engine);
                }
            }
            StudioAction::ToggleLearn => {
                self.studio.learn.toggle();
                self.say(if self.studio.learn.on {
                    "LEARN : clique un potard (VOL, SWING, ECHO, SPACE, EQ) puis bouge un potard de ton contrôleur MIDI."
                } else {
                    "LEARN coupé : les potards liés pilotent LOOM."
                });
            }
            StudioAction::ArmLearn(knob) => {
                if self.studio.learn.arm(knob) {
                    self.say(format!("LEARN : bouge un potard MIDI pour lier « {} ».", knob.label()));
                }
            }
            StudioAction::Tap => self.tap_tempo(),
            StudioAction::Stems => self.start_stems(ctx),
            StudioAction::Share => {
                let code = share::export(&self.text, &self.studio.knobs);
                let length = code.len();
                ctx.copy_text(code);
                self.say(format!("Code LOOM1 copié dans le presse-papiers ({length} caractères) : colle-le dans « Importer » ou dans le navigateur."));
            }
            StudioAction::OpenImport => self.studio.import = Some(String::new()),
            StudioAction::CloseImport => self.studio.import = None,
            StudioAction::Import(pasted) => self.import(&pasted),
            StudioAction::SelectSection(section) => self.studio.section = section,
            StudioAction::EditSong(edit) => {
                let Some(selected) = self.studio.section else { return };
                let (song, selected) = songline::edit_sections(&songline::sections(&self.text), selected, edit);
                self.studio.section = selected;
                self.text = songline::set_song(&self.text, &song);
                self.apply_text_change();
                let total: u32 = song.iter().map(|(_, bars)| bars).sum();
                self.say(format!("Arrangement : {} sections, {total} mesures.", song.len()));
            }
            StudioAction::GoToVoice(voice) => self.go_to_voice(&voice, ctx),
            StudioAction::Clip(voice, verb) => {
                self.studio.clips.set(&voice, verb);
                let what = if verb.is_empty() { "comme écrit".to_string() } else { format!("| {verb}") };
                if self.is_playing() {
                    self.run();
                    self.say(format!("Clip {voice} : {what} (à la prochaine mesure)."));
                } else {
                    self.say(format!("Clip {voice} : {what}. Ctrl+Entrée pour l'entendre."));
                }
            }
            StudioAction::Launch(path) => {
                self.open(&path);
                self.run();
            }
            StudioAction::ToggleDemo => {
                if self.studio.demo.is_some() {
                    self.stop_demo();
                } else {
                    self.start_demo();
                }
            }
        }
    }

    /// Work done every frame: meters and loudness, MIDI LEARN, stems progress, the demo.
    pub(super) fn studio_frame(&mut self, ctx: &egui::Context) {
        let dt = self.studio.last_frame.elapsed().as_secs_f32();
        self.studio.last_frame = std::time::Instant::now();
        let now = self.studio.clock.elapsed().as_secs_f64();

        let mut learned = LearnEvents::default();
        if let Some(engine) = &self.engine {
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
                learned = self.studio.learn.handle(&controls, &mut self.studio.knobs);
                for knob in &learned.changed {
                    self.studio.knobs.apply(*knob, engine);
                }
            }
        }
        if let Some((cc, knob)) = learned.bound {
            self.say(format!("LEARN : CC{cc} → {} (enregistré).", knob.label()));
        }
        if let Some(error) = learned.error {
            self.warn(error);
        }

        if let Some(result) = self.studio.stems.as_mut().and_then(StemsJob::poll) {
            self.studio.stems = None;
            match result {
                Ok((dir, count)) => self.say(format!("STEMS : {count} pistes exportées dans {}", dir.display())),
                Err(e) => self.warn(e),
            }
        }

        if let Some(next) = self.studio.demo.as_mut().and_then(|run| run.demo.next_if_due().map(Path::to_path_buf)) {
            self.demo_play(&next);
        }
        // MIDI knobs and the demo need the app awake even when nothing moves on screen
        if self.studio.learn.on || !self.studio.learn.bindings.pairs.is_empty() || self.studio.demo.is_some() {
            ctx.request_repaint_after(std::time::Duration::from_millis(100));
        }
    }

    /// Any key or click stops the demo. Returns the action to do it, if needed.
    pub(super) fn demo_interrupted(&mut self, ctx: &egui::Context) -> Option<Action> {
        self.studio.demo.as_ref()?;
        let pressed = ctx.input(|i| {
            i.events.iter().any(|e| matches!(e, egui::Event::Key { pressed: true, .. } | egui::Event::PointerButton { pressed: true, .. }))
        });
        if pressed {
            ctx.input_mut(|i| i.events.clear()); // this key only stops the demo
            Some(Action::Studio(StudioAction::ToggleDemo))
        } else {
            None
        }
    }

    /// The knobs bar (when shown).
    pub(super) fn studio_bar(&mut self, ui: &mut egui::Ui, actions: &mut Vec<Action>) {
        if !self.studio.show_panel {
            return;
        }
        let mut studio_actions = Vec::new();
        let theme = self.theme.clone();
        let bpm = self.program.bpm;
        studio_ui::studio_bar(ui, &mut self.studio, &theme, bpm, self.engine.is_some(), &mut studio_actions);
        actions.extend(studio_actions.into_iter().map(Action::Studio));
    }

    /// The studio panels between the grid and the editor: ROLL, MIX, CLIPS, TIMELINE.
    pub(super) fn studio_panels(&mut self, ui: &mut egui::Ui, actions: &mut Vec<Action>) {
        let theme = self.theme.clone();
        let status = self.engine.as_ref().map(|e| e.status()).filter(|s| s.playing());
        let mut studio_actions = Vec::new();
        if self.studio.show_roll {
            let bar = status.map_or(0, |s| s.song_bar());
            let notes = roll::roll(&self.program, bar);
            studio_ui::roll::show(ui, &notes, bar, status.map(|s| s.phase()), &theme, &mut studio_actions);
        }
        if self.studio.show_notation {
            let bar = status.map_or(0, |s| s.song_bar());
            let (notes, _) = loom_app::studio::notation::staff(&roll::roll(&self.program, bar));
            let armed = self.studio.notation_voice.clone();
            let semi = armed
                .as_deref()
                .and_then(|v| self.program.voices.iter().find(|x| x.name == v).map(|x| x.synth.semi.unwrap_or(0)))
                .unwrap_or(0);
            studio_ui::notation::show(ui, &notes, bar, armed.as_deref(), semi, &theme, &mut studio_actions);
        }
        if self.studio.show_sampler {
            studio_ui::sampler::show(ui, &mut self.studio, &self.program.voices, &theme, &mut studio_actions);
        }
        if self.studio.show_mixer {
            let peak = status.map_or(0.0, |s| s.peak());
            studio_ui::mixer::show(ui, &mut self.studio, &self.muted, peak, &theme, actions);
        }
        if self.studio.show_clips {
            let songs = demo::playlist(&self.library);
            studio_ui::clips::show(ui, &self.studio, &songs, &theme, &mut studio_actions);
        }
        let song = songline::sections(&self.text);
        if !song.is_empty() {
            let position = status.map(|s| s.song_bar() as f32 + s.phase());
            studio_ui::timeline::show(ui, &song, self.studio.section.filter(|i| *i < song.len()), position, &theme, &mut studio_actions);
        }
        actions.extend(studio_actions.into_iter().map(Action::Studio));
    }

    /// The import box, while it's open.
    pub(super) fn import_dialog(&mut self, ctx: &egui::Context, actions: &mut Vec<Action>) {
        let Some(text) = self.studio.import.as_mut() else { return };
        let mut studio_actions = Vec::new();
        studio_ui::import_dialog(ctx, text, &mut studio_actions);
        actions.extend(studio_actions.into_iter().map(Action::Studio));
    }

    /// The editor text was changed by a studio button: the grid follows,
    /// and while playing the change is heard at the next bar.
    fn apply_text_change(&mut self) {
        if self.is_playing() {
            self.run();
        } else {
            self.set_program(Program::parse(&self.text));
        }
    }

    /// TAP: after two taps, the `bpm:` line gets the tapped tempo.
    fn tap_tempo(&mut self) {
        match self.studio.tap.tap() {
            Some(bpm) => {
                self.text = songline::set_bpm(&self.text, bpm);
                let message = format!("TAP : {bpm} BPM (ligne bpm: réécrite).");
                self.apply_text_change();
                self.say(message);
            }
            None => self.say("TAP : continue de taper en rythme…"),
        }
    }

    /// PARTITION ◀/▶: transpose the armed voice by `dt` semitones, undoably.
    fn notation_transpose(&mut self, dt: i32) {
        let Some(voice) = self.studio.notation_voice.clone() else {
            self.say("PARTITION : clique une note pour choisir la voix à transposer.");
            return;
        };
        match loom_app::studio::notation::transpose_voice(&self.text, &voice, dt) {
            Ok(code) => {
                let when = self.replace_song("TRANSPOSE", code);
                self.say(format!("PARTITION : « {voice} » {dt:+} demi-ton(s){when}."));
            }
            Err(e) => self.warn(format!("PARTITION : {e}")),
        }
    }

    /// SAMPLER APPLIQUER: write the draft (sample, root, window, chop,
    /// reverse) onto the armed voice's line, undoably.
    fn sampler_apply(&mut self) {
        let Some(voice) = self.studio.sampler_voice.clone() else {
            self.say("SAMPLER : choisis d'abord une voix (clic sur son chip).");
            return;
        };
        match loom_app::studio::sampletool::apply_spec(&self.text, &voice, &self.studio.sampler_spec) {
            Ok(code) => {
                let when = self.replace_song("SAMPLER", code);
                let sample = self.studio.sampler_spec.sample.clone().unwrap_or_default();
                self.say(if sample.is_empty() {
                    format!("SAMPLER : « {voice} » redevient une voix mélodique{when}.")
                } else {
                    format!("SAMPLER : « {voice} » joue « {sample} »{when}.")
                });
            }
            Err(e) => self.warn(format!("SAMPLER : {e}")),
        }
    }

    /// SAMPLER : load a user WAV into the session bank and arm it on the voice.
    fn sampler_load_wav(&mut self, path: &std::path::Path) {
        match loom_audio::load_user_sample(path) {
            Ok(name) => {
                self.studio.sampler_spec.sample = Some(name.clone());
                if let Some(parent) = path.parent() {
                    self.studio.sampler_dir = parent.to_path_buf();
                }
                self.studio.sampler_msg = Some(format!(
                    "« {name} » chargé ({}) — APPLIQUER pour l'écrire sur la voix armée.",
                    loom_audio::session_sample_count()
                ));
                if let Some(v) = self.studio.sampler_voice.clone() {
                    self.say(format!("SAMPLER : « {v} » jouera « {name} » — APPLIQUER."));
                } else {
                    self.say(format!("SAMPLER : « {name} » chargé — arme une voix puis APPLIQUER."));
                }
            }
            Err(e) => self.studio.sampler_msg = Some(e),
        }
    }

    fn start_stems(&mut self, ctx: &egui::Context) {
        if self.studio.stems.is_some() {
            return;
        }
        let sample_rate = self.engine.as_ref().map_or(48_000, |e| e.sample_rate());
        let wake = ctx.clone();
        match StemsJob::start(self.text.clone(), self.path.as_deref(), sample_rate, move || wake.request_repaint()) {
            Some(job) => {
                self.say(format!("STEMS : {} voix vers {}…", job.total, job.dir.display()));
                self.studio.stems = Some(job);
            }
            None => self.warn("STEMS : aucune voix à exporter."),
        }
    }

    /// Load a pasted code, link or LOOM code, with its knob values.
    fn import(&mut self, pasted: &str) {
        let Some(imported) = share::import(pasted, &self.studio.knobs) else {
            self.warn("Import : ce n'est ni un code LOOM1:, ni un lien LOOM, ni du code LOOM.");
            return;
        };
        self.studio.import = None;
        self.set_text(imported.code, None);
        self.saved_text = String::new(); // a new, unsaved song (Ctrl+S to keep it)
        self.studio.knobs = imported.knobs;
        if let Some(engine) = &self.engine {
            self.studio.knobs.apply_all(engine);
        }
        let knobs = if imported.has_knobs { " avec ses réglages de potards" } else { "" };
        self.say(format!("Morceau importé{knobs}. Ctrl+Entrée pour l'entendre, Ctrl+S pour l'enregistrer."));
    }

    /// Put the editor's cursor at the start of a voice's line.
    fn go_to_voice(&mut self, voice: &str, ctx: &egui::Context) {
        let Some(line) = songline::voice_line(&self.text, voice) else {
            self.warn(format!("La voix « {voice} » n'est pas dans le code de l'éditeur."));
            return;
        };
        let at: usize = self.text.split('\n').take(line).map(|l| l.chars().count() + 1).sum();
        let id = egui::Id::new(super::EDITOR_ID);
        let mut state = egui::text_edit::TextEditState::load(ctx, id).unwrap_or_default();
        let cursor = egui::text::CCursor::new(at);
        state.cursor.set_char_range(Some(egui::text_selection::CCursorRange::one(cursor)));
        state.store(ctx, id);
        ctx.memory_mut(|m| m.request_focus(id));
        self.say(format!("Ligne {} : {voice}", line + 1));
    }

    fn start_demo(&mut self) {
        let Some(tour) = demo::Demo::start(demo::playlist(&Library::scan())) else {
            self.warn("DÉMO : aucun morceau d'exemple trouvé (lance ./install.sh).");
            return;
        };
        let first = tour.current().to_path_buf();
        self.studio.demo = Some(DemoRun {
            demo: tour,
            saved_text: self.text.clone(),
            saved_disk_text: self.saved_text.clone(),
            saved_path: self.path.clone(),
            was_vj: self.stage.director.vj,
        });
        self.stage_full = true;
        if !self.stage.director.vj {
            self.stage.toggle_vj();
        }
        self.demo_play(&first);
    }

    /// Play one song of the demo, without touching the song library's "last file".
    fn demo_play(&mut self, path: &Path) {
        match std::fs::read_to_string(path) {
            Ok(text) => {
                self.set_text(text, Some(path.to_path_buf()));
                self.run();
                if let Some(run) = &self.studio.demo {
                    self.say(format!("{} — une touche ou un clic pour arrêter", run.demo.text()));
                }
            }
            Err(e) => self.warn(format!("DÉMO : impossible de lire {} : {e}", path.display())),
        }
    }

    /// Stop the demo and bring back the user's song as it was.
    pub(super) fn stop_demo(&mut self) {
        let Some(run) = self.studio.demo.take() else { return };
        self.stop();
        self.set_text(run.saved_text, run.saved_path);
        self.saved_text = run.saved_disk_text;
        self.set_program(Program::parse(&self.text));
        self.stage_full = false;
        if self.stage.director.vj != run.was_vj {
            self.stage.toggle_vj();
        }
        self.say("Démo arrêtée : ton morceau est revenu.");
    }
}
