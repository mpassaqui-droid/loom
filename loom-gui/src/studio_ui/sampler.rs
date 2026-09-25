//! SAMPLER panel: make any voice play a sample, the way the studio hears it —
//! a machine with its sample list, its mini-waveform and its knobs, docked on
//! the RIGHT of the window (the browser SAMPLE box sits on the right too).
//!
//! The list comes from `loom_audio::default_bank()` (the same bank the engine
//! uses), so it shows exactly the samples that will play. The panel edits a
//! *draft* (in `Studio.sampler_spec`); nothing hits the song until APPLIQUER,
//! which rewrites the armed voice's `| sample`, `| root`, `| begin/end`,
//! `| chop`, `| reverse` verbs (undoable, see the SAMPLER handler in
//! `app_studio.rs`). The rewriting logic lives in
//! [`loom_app::studio::sampletool`].

use eframe::egui::{self, Align2, Color32, FontId, RichText, Stroke};
use loom_app::Theme;

use super::{Studio, StudioAction};
use crate::scene::logic::{hsl_to_rgb, voice_hue};
use crate::style::{color, mix};

/// Draw the sampler, docked on the right. `voices` is the parsed song, so the
/// chips show exactly the voices that exist.
pub fn show(
    ui: &mut egui::Ui,
    studio: &mut Studio,
    voices: &[loom_core::Voice],
    theme: &Theme,
    actions: &mut Vec<StudioAction>,
) {
    egui::Panel::right("sampler")
        .resizable(true)
        .default_size(320.0)
        .frame(egui::Frame::new().fill(color(theme.panel)).inner_margin(8))
        .show(ui, |ui| {
            egui::ScrollArea::vertical()
                .id_salt("sampler_panel")
                .show(ui, |ui| {
                    ui.horizontal_wrapped(|ui| {
                        ui.label(RichText::new("Sampler").strong());
                        if let Some(voice) = &studio.sampler_voice {
                            ui.label(
                                RichText::new(format!("voix {voice}")).color(color(theme.accent)),
                            );
                            let reset = ui.button("réinit").on_hover_text(
                                "Repartir du réglage actuel de la voix (le code n'est pas modifié)",
                            );
                            if reset.clicked() {
                                actions.push(StudioAction::ArmSampler(voice.clone()));
                            }
                        }
                    });
                    ui.label(
                        RichText::new("choisis une voix, un échantillon, ajuste, puis APPLIQUER.")
                            .small()
                            .color(color(theme.dim)),
                    );

                    if voices.is_empty() {
                        ui.label(
                            RichText::new("Aucune voix : écris une voix puis Ctrl+Entrée.")
                                .color(color(theme.dim)),
                        );
                        return;
                    }

                    // one chip per voice; the armed one is lit, the ones that already
                    // use a sample are emphasised.
                    ui.horizontal_wrapped(|ui| {
                        for v in voices {
                            let armed = studio.sampler_voice.as_deref() == Some(v.name.as_str());
                            let (r, g, b) = hsl_to_rgb(voice_hue(&v.name, None), 0.6, 0.72);
                            let ink = Color32::from_rgb(r, g, b);
                            let text = RichText::new(&v.name).monospace();
                            let text = if v.synth.sample.is_some() {
                                text.strong()
                            } else {
                                text
                            };
                            let mut btn = egui::Button::new(text);
                            if armed {
                                btn = btn
                                    .fill(ink)
                                    .stroke(Stroke::new(1.0, color(theme.foreground)));
                            }
                            let b = ui
                                .add(btn)
                                .on_hover_text(format!("Voix « {} » — clic pour l'armer", v.name));
                            if b.clicked() {
                                actions.push(StudioAction::ArmSampler(v.name.clone()));
                            }
                        }
                    });
                    ui.add_space(4.0);

                    let bank = loom_audio::default_bank();
                    egui::ScrollArea::vertical()
                        .id_salt("sampler_sample_list")
                        .max_height(180.0)
                        .auto_shrink([false, false])
                        .show(ui, |ui| sample_rows(ui, studio, &bank, theme));

                    wave(ui, studio, &bank, theme);
                    ui.add_space(4.0);
                    controls(ui, studio, theme, actions);
                });
        });
}

/// One selectable row per sample of the bank, with its length.
fn sample_rows(
    ui: &mut egui::Ui,
    studio: &mut Studio,
    bank: &loom_audio::sample::SampleBank,
    theme: &Theme,
) {
    let names = bank.names();
    if names.is_empty() {
        ui.label(RichText::new("aucun WAV trouvé").color(color(theme.dim)));
        return;
    }
    for name in &names {
        let selected = studio.sampler_spec.sample.as_deref() == Some(name.as_str());
        let dur = bank
            .get(name)
            .map(|d| format!("{:.2}s", d.duration()))
            .unwrap_or_default();
        let text = RichText::new(format!("{name}  {dur}"))
            .monospace()
            .size(11.0);
        let btn = if selected {
            egui::Button::new(text.color(color(theme.background))).fill(color(theme.accent))
        } else {
            egui::Button::new(text)
        };
        if ui
            .add(btn)
            .on_hover_text(format!("Utiliser « {name} » pour la voix armée"))
            .clicked()
        {
            studio.sampler_spec.sample = Some(name.clone());
        }
    }
}

/// The miniature waveform of the chosen sample with begin/end markers.
fn wave(
    ui: &mut egui::Ui,
    studio: &mut Studio,
    bank: &loom_audio::sample::SampleBank,
    theme: &Theme,
) {
    let Some(name) = studio.sampler_spec.sample.clone() else {
        ui.label(
            RichText::new("clique un échantillon pour voir sa forme d'onde.")
                .small()
                .color(color(theme.dim)),
        );
        return;
    };
    let Some(data) = bank.get(&name) else {
        ui.label(
            RichText::new(format!("« {name} » introuvable."))
                .small()
                .color(color(theme.error)),
        );
        return;
    };
    let (rect, _) =
        ui.allocate_exact_size(egui::vec2(ui.available_width(), 56.0), egui::Sense::hover());
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, 3.0, color(theme.background));
    let ch = &data.channels[0];
    if !ch.is_empty() {
        let ink = mix(theme.background, theme.foreground, 0.30);
        let buckets = rect.width().max(1.0) as usize;
        let bw = rect.width() / buckets as f32;
        for b in 0..buckets {
            let lo = (b * ch.len() / buckets).min(ch.len() - 1);
            let hi = (((b + 1) * ch.len()) / buckets).max(lo + 1).min(ch.len());
            let mut max = 0.0f32;
            let mut min = 0.0f32;
            for s in &ch[lo..hi] {
                if *s > max {
                    max = *s;
                }
                if *s < min {
                    min = *s;
                }
            }
            let y_top = rect.center().y - max * rect.height() * 0.5;
            let y_bot = rect.center().y - min * rect.height() * 0.5;
            painter.rect_filled(
                egui::Rect::from_min_max(
                    egui::pos2(rect.left() + b as f32 * bw, y_top),
                    egui::pos2(rect.left() + (b as f32 + 1.0) * bw, y_bot),
                ),
                0.0,
                ink,
            );
        }
    }
    let begin = studio.sampler_spec.begin.unwrap_or(0.0);
    let end = studio.sampler_spec.end.unwrap_or(1.0);
    for (t, lab) in [(begin, "begin"), (end, "end")] {
        let x = rect.left() + t.clamp(0.0, 1.0) * rect.width();
        let ink = if lab == "begin" {
            color(theme.accent)
        } else {
            color(theme.note)
        };
        painter.vline(x, rect.y_range(), Stroke::new(1.5, ink));
        painter.text(
            egui::pos2(x + 2.0, rect.bottom() - 2.0),
            Align2::LEFT_BOTTOM,
            lab,
            FontId::monospace(9.0),
            color(theme.dim),
        );
    }
}

/// The voice spec: root, reverse, the begin/end/chop window, and APPLIQUER.
fn controls(
    ui: &mut egui::Ui,
    studio: &mut Studio,
    theme: &Theme,
    actions: &mut Vec<StudioAction>,
) {
    ui.horizontal(|ui| {
        ui.label(RichText::new("racine").small().color(color(theme.dim)));
        let mut root = studio.sampler_spec.root.clone().unwrap_or_default();
        let resp = ui.add(
            egui::TextEdit::singleline(&mut root)
                .hint_text("c3")
                .desired_width(56.0)
                .font(egui::TextStyle::Monospace),
        );
        if resp.changed() {
            if root.trim().is_empty() {
                studio.sampler_spec.root = None;
            } else {
                studio.sampler_spec.root = Some(root.trim().to_string());
            }
        }
        ui.checkbox(&mut studio.sampler_spec.reverse, "reverse");
        ui.label(
            RichText::new("lit à l'envers")
                .small()
                .color(color(theme.dim)),
        );
    });
    let mut begin = studio.sampler_spec.begin.unwrap_or(0.0);
    ui.horizontal(|ui| {
        ui.label(RichText::new("begin").monospace().small());
        if ui
            .add(egui::Slider::new(&mut begin, 0.0..=1.0).show_value(true))
            .on_hover_text("où la lecture commence")
            .changed()
        {
            studio.sampler_spec.begin = Some(begin);
        }
    });
    let mut end = studio.sampler_spec.end.unwrap_or(1.0);
    ui.horizontal(|ui| {
        ui.label(RichText::new("end").monospace().small());
        if ui
            .add(egui::Slider::new(&mut end, 0.0..=1.0).show_value(true))
            .on_hover_text("où la lecture s'arrête")
            .changed()
        {
            studio.sampler_spec.end = Some(end);
        }
    });
    let mut chop = studio.sampler_spec.chop.unwrap_or(0).min(16) as u8;
    ui.horizontal(|ui| {
        ui.label(RichText::new("chop").monospace().small());
        if ui
            .add(egui::Slider::new(&mut chop, 0..=16).show_value(true))
            .on_hover_text("tranches (0 = off)")
            .changed()
        {
            studio.sampler_spec.chop = if chop == 0 {
                None
            } else {
                Some(u32::from(chop))
            };
        }
    });
    if ui.button(RichText::new("APPLIQUER").strong()).clicked() {
        actions.push(StudioAction::SamplerApply);
    }
    ui.label(
        RichText::new("réécrit la voix armée (annulable) · Ctrl+Entrée pour l'entendre.")
            .small()
            .color(color(theme.dim)),
    );
}
