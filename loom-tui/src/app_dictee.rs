//! La dictée de VIBE (Ctrl+D dans VIBE, ou Alt+V) : on parle, whisper écrit la
//! demande en local, puis Entrée l'envoie. Rien ne quitte la machine.

use super::{App, Mode};
use loom_app::dictee::Dictation;
use loom_audio::input::MicInput;
use std::time::{Duration, Instant};

/// Au-delà, le micro s'arrête tout seul (le tampon garde 30 s).
const MAX: Duration = Duration::from_secs(28);

#[derive(Default)]
pub enum Dictee {
    #[default]
    Off,
    Listening { mic: MicInput, since: Instant },
    Writing { job: Dictation, since: Instant },
}

impl App {
    /// Alt+V : ouvre VIBE et écoute tout de suite.
    pub fn dictate(&mut self) {
        if !matches!(self.mode, Mode::Vibe { .. }) { self.mode = Mode::Vibe { input: String::new() }; }
        if matches!(self.dictee, Dictee::Off) { self.toggle_dictee(); }
    }

    /// Ctrl+D dans VIBE : écouter, puis arrêter et faire écrire whisper.
    pub fn toggle_dictee(&mut self) {
        match std::mem::take(&mut self.dictee) {
            Dictee::Off => match MicInput::start() {
                Ok(mic) => {
                    self.info(format!("🎙 j'écoute ({}) · parle, puis Ctrl+D", mic.device_name()));
                    self.dictee = Dictee::Listening { mic, since: Instant::now() };
                }
                Err(e) => self.error(format!("dictée : {e}")),
            },
            Dictee::Listening { mic, .. } => self.stop_listening(mic),
            writing @ Dictee::Writing { .. } => self.dictee = writing,
        }
    }

    fn stop_listening(&mut self, mic: MicInput) {
        let (samples, rate) = (mic.take(), mic.sample_rate());
        drop(mic); // le micro se ferme tout de suite
        match Dictation::start(samples, rate) {
            Ok(job) => self.dictee = Dictee::Writing { job, since: Instant::now() },
            Err(e) => self.error(format!("dictée : {e}")),
        }
    }

    /// À chaque image : arrêt automatique, et le texte de whisper dans VIBE.
    pub(super) fn dictee_tick(&mut self) {
        match &mut self.dictee {
            Dictee::Listening { since, .. } if since.elapsed() > MAX => {
                if let Dictee::Listening { mic, .. } = std::mem::take(&mut self.dictee) { self.stop_listening(mic); }
            }
            Dictee::Writing { job, .. } => {
                let Some(result) = job.poll() else { return };
                self.dictee = Dictee::Off;
                match result {
                    Ok(text) => {
                        match &mut self.mode {
                            Mode::Vibe { input } => {
                                if !input.is_empty() && !input.ends_with(' ') { input.push(' '); }
                                input.push_str(&text);
                            }
                            _ => self.mode = Mode::Vibe { input: text },
                        }
                        self.info("dictée écrite · Entrée l'envoie à VIBE, Ctrl+D pour en dire plus");
                    }
                    Err(e) => self.error(format!("dictée : {e}")),
                }
            }
            _ => {}
        }
    }

    /// Ce que la ligne de VIBE affiche pendant la dictée.
    pub fn dictee_label(&self) -> Option<String> {
        match &self.dictee {
            Dictee::Off => None,
            Dictee::Listening { mic, since } => {
                let bars = (mic.level() * 40.0).clamp(0.0, 8.0) as usize;
                Some(format!("🎙 {:>2}s {}{} Ctrl+D fini", since.elapsed().as_secs(), "▮".repeat(bars), "▯".repeat(8 - bars)))
            }
            Dictee::Writing { since, .. } => Some(format!("✍ whisper écrit… {}s", since.elapsed().as_secs())),
        }
    }
}
