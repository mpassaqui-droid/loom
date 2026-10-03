//! The STUDIO in the window: the knobs bar and the panels (MIX, ROLL,
//! PARTITION, TIMELINE, CLIPS), plus the import box.
//!
//! The logic lives in [`loom_app::studio`]; these functions only draw it and
//! turn clicks into [`StudioAction`]s, which the app carries out
//! (see `app_studio.rs`).

pub mod clips;
pub mod mixer;
pub mod notation;
pub mod roll;
pub mod sampler;
pub mod timeline;

use std::path::PathBuf;
use std::time::Instant;

use eframe::egui::{self, Button, Id, RichText, Sense};
use loom_app::Theme;
use loom_audio::SCOPE_LEN;

use loom_app::studio::clips::Clips;
use loom_app::studio::demo::Demo;
use loom_app::studio::knobs::{Knob, Knobs};
use loom_app::studio::learn::Learn;
use loom_app::studio::loudness::{Level, Loudness};
use loom_app::studio::mixer::Mixer;
use loom_app::studio::sampletool::SampleSpec;
use loom_app::studio::songline::SongEdit;
use loom_app::studio::stems::StemsJob;
use loom_app::studio::tap::TapTempo;
use crate::style::color;

/// Room one knob needs in the knobs bar (name, slider, value), in points.
const KNOB_WIDTH: f32 = 230.0;

/// Something asked for in the studio.
#[derive(Debug, Clone, PartialEq)]
pub enum StudioAction {
    /// Show / hide the knobs bar (F6).
    TogglePanel,
    ToggleMixer,
    ToggleRoll,
    ToggleClips,
    /// Show / hide the PARTITION (grand staff) panel.
    ToggleNotation,
    /// Transpose the armed voice by `n` semitones (`| semi` is rewritten).
    NotationTranspose(i32),
    /// Click a note on the staff: this voice gets the ◀ ▶ buttons.
    ArmNotation(String),
    /// Show / hide the SAMPLER panel.
    ToggleSampler,
    /// Arm a voice for the sampler and reload its current spec.
    ArmSampler(String),
    /// Write the sampler draft onto the armed voice's line.
    SamplerApply,
    /// Navigate the sampler's file browser to `dir`.
    SamplerDir(String),
    /// Load a user WAV (path) into the session bank and arm it on the voice.
    SamplerLoadWav(std::path::PathBuf),
    /// Re-scan the sample folders on disk (keeps session files).
    SamplerReloadBank,
    /// A knob was moved on screen: send it to the engine.
    KnobChanged(Knob),
    /// Mixer strip `n` changed its fader or pan.
    StripChanged(usize),
    /// A `send:` return fader moved.
    SendLevel(usize),
    ToggleSolo(usize),
    ToggleLearn,
    /// While LEARN is on: this knob waits for a controller move.
    ArmLearn(Knob),
    /// TAP tempo (F7 or the button).
    Tap,
    Stems,
    /// Copy a `LOOM1:` code to the clipboard.
    Share,
    OpenImport,
    CloseImport,
    /// Load what was pasted in the import box.
    Import(String),
    /// Pick a section on the timeline (or none).
    SelectSection(Option<usize>),
    EditSong(SongEdit),
    /// Put the cursor on this voice's line in the editor.
    GoToVoice(String),
    /// Play voice `.0` with the CLIPS slot verb `.1` (`""` = as written).
    Clip(String, &'static str),
    /// Open a song and play it (at the next bar if already playing).
    Launch(PathBuf),
    ToggleDemo,
}

/// The demo tour, and the song to bring back when it stops.
pub struct DemoRun {
    pub demo: Demo,
    pub saved_text: String,
    pub saved_disk_text: String,
    pub saved_path: Option<PathBuf>,
    pub was_vj: bool,
}

/// Everything the studio remembers.
pub struct Studio {
    pub knobs: Knobs,
    pub mixer: Mixer,
    pub tap: TapTempo,
    pub loudness: Loudness,
    pub learn: Learn,
    pub stems: Option<StemsJob>,
    pub clips: Clips,
    pub demo: Option<DemoRun>,
    pub show_panel: bool,
    pub show_mixer: bool,
    pub show_roll: bool,
    pub show_clips: bool,
    pub show_notation: bool,
    pub show_sampler: bool,
    /// The voice the PARTITION panel's ◀ ▶ transpose.
    pub notation_voice: Option<String>,
    /// The voice the SAMPLER panel writes on.
    pub sampler_voice: Option<String>,
    /// The sampler draft; only APPLIQUER writes it into the song.
    pub sampler_spec: SampleSpec,
    /// Where the SAMPLER panel's browse-to-your-files navigator is.
    pub sampler_dir: std::path::PathBuf,
    /// Last message about a loaded / reloaded bank (path wrong, file refused…).
    pub sampler_msg: Option<String>,
    /// The import box, open with what was pasted so far.
    pub import: Option<String>,
    /// The section selected on the timeline.
    pub section: Option<usize>,
    /// Copy of the output for the loudness meter.
    pub scope: Box<[f32; SCOPE_LEN]>,
    pub last_frame: Instant,
    /// When the studio started, the clock of the loudness meter.
    pub clock: Instant,
}

impl Studio {
    pub fn new(sample_rate: u32) -> Studio {
        Studio {
            knobs: Knobs::default(),
            mixer: Mixer::default(),
            tap: TapTempo::default(),
            loudness: Loudness::new(sample_rate),
            learn: Learn::load(),
            stems: None,
            clips: Clips::default(),
            demo: None,
            show_panel: false,
            show_mixer: false,
            show_roll: false,
            show_clips: false,
            show_notation: false,
            show_sampler: false,
            sampler_dir: default_sampler_dir(),
            notation_voice: None,
            sampler_voice: None,
            sampler_spec: SampleSpec::default(),
            sampler_msg: None,
            import: None,
            section: None,
            scope: Box::new([0.0; SCOPE_LEN]),
            last_frame: Instant::now(),
            clock: Instant::now(),
        }
    }
}

/// Where the SAMPLER file navigator starts: the user's sample folder, else the
/// bundled `web/samples`, else the home directory.
pub fn default_sampler_dir() -> std::path::PathBuf {
    let candidates = [
        loom_audio::paths::data_dir().map(|d| d.join("samples")),
        std::env::var_os("LOOM_SAMPLES").map(std::path::PathBuf::from),
        std::env::current_dir().ok().map(|c| c.join("web/samples")),
        loom_audio::paths::home(),
    ];
    candidates.into_iter().flatten().find(|p| p.is_dir()).unwrap_or_else(|| std::path::PathBuf::from("."))
}

/// `LUFS~ -14 · pk -3dB`, coloured like the browser (amber when loud, red when clipping).
pub fn loudness_label(ui: &mut egui::Ui, loudness: &Loudness, theme: &Theme) {
    let tint = match loudness.level() {
        Level::Clipping => color(theme.error),
        Level::Loud => color(theme.drum),
        Level::Normal => color(theme.dim),
    };
    ui.label(RichText::new(loudness.text()).monospace().size(13.0).color(tint))
        .on_hover_text("Loudness à court terme (LUFS~, cible streaming ≈ -14) · crête en dB · rouge = écrêtage");
}

/// The knobs bar: SWING, ECHO, SPACE, VOL, EQ, and the studio buttons.
pub fn studio_bar(ui: &mut egui::Ui, studio: &mut Studio, theme: &Theme, tap_bpm: f64, has_engine: bool, actions: &mut Vec<StudioAction>) {
    egui::Panel::top("studio")
        .frame(egui::Frame::new().fill(color(theme.panel)).inner_margin(egui::Margin::symmetric(8, 6)))
        .show(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Extend);
                for knob in Knob::ALL {
                    // name, slider and value stay together: start a new row when they don't fit
                    let room = ui.max_rect().right() - ui.cursor().min.x;
                    if room < KNOB_WIDTH {
                        ui.end_row();
                    }
                    knob_widget(ui, studio, knob, theme, actions);
                }
            });
            ui.horizontal_wrapped(|ui| {
                ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Extend);
                let taps = studio.tap.count();
                let tap_text = if taps >= 2 { format!("◎ TAP {tap_bpm:.0}") } else { "◎ TAP".to_string() };
                if ui.button(tap_text).on_hover_text("Tape en rythme pour régler le tempo : la ligne bpm: est réécrite (F7)").clicked() {
                    actions.push(StudioAction::Tap);
                }
                match &studio.stems {
                    Some(job) => {
                        ui.add(egui::ProgressBar::new(job.fraction()).desired_width(90.0).show_percentage());
                        ui.label(RichText::new(job.progress_text()).color(color(theme.dim)));
                    }
                    None => {
                        if ui.button("⬇ STEMS").on_hover_text("Exporter chaque voix en WAV séparé, dans le dossier <morceau>-stems à côté du morceau").clicked() {
                            actions.push(StudioAction::Stems);
                        }
                    }
                }
                if has_engine {
                    let learn_label = if studio.learn.on { RichText::new("LEARN ⏺").color(color(theme.accent)) } else { RichText::new("LEARN") };
                    if ui.selectable_label(studio.learn.on, learn_label)
                        .on_hover_text("MIDI LEARN : active, clique un potard (VOL, EQ, SWING, ECHO, SPACE), puis bouge un potard de ton contrôleur")
                        .clicked()
                    {
                        actions.push(StudioAction::ToggleLearn);
                    }
                }
                ui.separator();
                if ui.button("Partager").on_hover_text("Copier un code LOOM1: (morceau + potards) dans le presse-papiers : il s'ouvre aussi dans le navigateur (Ctrl+Maj+L)").clicked() {
                    actions.push(StudioAction::Share);
                }
                if ui.button("Importer").on_hover_text("Coller un code LOOM1:, un lien du navigateur ou du code LOOM (Ctrl+Maj+I)").clicked() {
                    actions.push(StudioAction::OpenImport);
                }
                ui.separator();
                if ui.selectable_label(studio.show_mixer, "MIX").on_hover_text("Mixeur : volume, pan, mute, solo et niveau par voix (F8)").clicked() {
                    actions.push(StudioAction::ToggleMixer);
                }
                if ui.selectable_label(studio.show_roll, "ROLL").on_hover_text("Piano-roll : les mélodies en blocs, clic = aller à la ligne de la voix (F9)").clicked() {
                    actions.push(StudioAction::ToggleRoll);
                }
                if ui.selectable_label(studio.show_notation, "PARTITION").on_hover_text("Portée : le morceau en solfège (clé de sol + clé de fa), clic une note = aller à sa voix, ◀/▶ = transposer").clicked() {
                    actions.push(StudioAction::ToggleNotation);
                }
                if ui.selectable_label(studio.show_sampler, "SAMPLER").on_hover_text("Faire jouer un échantillon (web/samples) par n'importe quelle voix : sample, root, begin/end, chop, reverse — APPLIQUER réécrit le code (annulable)").clicked() {
                    actions.push(StudioAction::ToggleSampler);
                }
                if ui.selectable_label(studio.show_clips, "CLIPS").on_hover_text("Lancer un morceau, ou varier une voix en direct sans toucher au code (F10)").clicked() {
                    actions.push(StudioAction::ToggleClips);
                }
                if ui.selectable_label(studio.demo.is_some(), "DÉMO").on_hover_text("Visite guidée : enchaîne les morceaux d'exemple avec la scène, une touche ou un clic pour arrêter (Ctrl+Maj+D)").clicked() {
                    actions.push(StudioAction::ToggleDemo);
                }
            });
        });
}

/// One knob: its name (a click target for MIDI LEARN), a slider and the value.
fn knob_widget(ui: &mut egui::Ui, studio: &mut Studio, knob: Knob, theme: &Theme, actions: &mut Vec<StudioAction>) {
    let armed = studio.learn.armed == Some(knob);
    let bound = studio.learn.bindings.describe(knob);
    let mut name = RichText::new(knob.label()).strong();
    if armed {
        name = name.color(color(theme.accent));
    } else if bound.is_some() {
        name = name.color(color(theme.note));
    }
    let label = ui.add(egui::Label::new(name).sense(Sense::click()));
    let hover = match (studio.learn.on, armed, &bound) {
        (_, true, _) => "Bouge un potard MIDI maintenant…".to_string(),
        (true, false, _) => "Clic : lier ce potard à un contrôleur MIDI".to_string(),
        (false, _, Some(cc)) => format!("Piloté par {cc}"),
        (false, _, None) => "Active LEARN pour le piloter en MIDI".to_string(),
    };
    if label.on_hover_text(hover).clicked() && studio.learn.on {
        actions.push(StudioAction::ArmLearn(knob));
    }
    let (min, max, step) = knob.range();
    let slider = egui::Slider::new(studio.knobs.value_mut(knob), min..=max).step_by(step as f64).show_value(false);
    let response = ui.add_sized(egui::vec2(90.0, 18.0), slider);
    if response.clicked() && studio.learn.on {
        actions.push(StudioAction::ArmLearn(knob));
    }
    if response.changed() {
        actions.push(StudioAction::KnobChanged(knob));
    }
    ui.label(RichText::new(knob.format(studio.knobs.get(knob))).monospace().color(color(theme.accent)));
    ui.add_space(6.0);
}

/// The import box. Returns what to do once a button is pressed.
pub fn import_dialog(ctx: &egui::Context, text: &mut String, actions: &mut Vec<StudioAction>) {
    let response = egui::Modal::new(Id::new("studio-import")).show(ctx, |ui| {
        ui.set_width(480.0);
        ui.heading("Importer un morceau");
        ui.label("Colle un code LOOM1:, un lien du navigateur (…#s=…) ou directement du code LOOM.");
        let field = ui.add(egui::TextEdit::multiline(text).desired_rows(6).desired_width(f32::INFINITY).hint_text("LOOM1:…  (Ctrl+V pour coller)"));
        if text.is_empty() && !field.has_focus() {
            field.request_focus(); // ready for Ctrl+V as soon as the box opens
        }
        let confirm = field.has_focus() && ui.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, egui::Key::Enter));
        ui.horizontal(|ui| {
            let import = ui.add_enabled(!text.trim().is_empty(), Button::new("Importer")).on_hover_text("Charger ce morceau (Ctrl+Entrée)");
            if (import.clicked() || confirm) && !text.trim().is_empty() {
                actions.push(StudioAction::Import(text.clone()));
            }
            if ui.button("Annuler").clicked() {
                actions.push(StudioAction::CloseImport);
            }
        });
    });
    if response.should_close() && actions.is_empty() {
        actions.push(StudioAction::CloseImport);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use loom_app::studio::clips::SLOTS;

    /// Every symbol the studio draws must exist in egui's bundled fonts.
    #[test]
    fn studio_symbols_exist_in_the_bundled_fonts() {
        let ctx = egui::Context::default();
        let mut output = ctx.run_ui(egui::RawInput::default(), |_| {});
        let font = egui::FontId::proportional(14.0);
        let missing = |c: char| {
            let uv = |ch: char| ctx.fonts_mut(|f| f.layout_no_wrap(ch.to_string(), font.clone(), egui::Color32::WHITE).rows[0].row.glyphs[0].uv_rect);
            let (glyph, boxed) = (uv(c), uv('◻'));
            (glyph.min, glyph.max) == (boxed.min, boxed.max)
        };
        let labels: String = SLOTS.iter().map(|(l, _)| *l).collect();
        let absent: String = format!("◎⬇⏺◀▶×−∞~·↔{labels}").chars().filter(|c| missing(*c)).collect();
        output.textures_delta.clear(); // no GPU here: nothing to upload
        assert!(absent.is_empty(), "no glyph for {absent}");
    }
}
