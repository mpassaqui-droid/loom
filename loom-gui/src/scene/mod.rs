//! The SCÈNE: the browser's full-screen visuals, drawn natively with egui.
//!
//! - [`logic`]: what to show and the sound analysis (pure, tested; shared with the TUI).
//! - [`pen`]: triangles with glow and gradients.
//! - [`scenes`]: the scenes themselves, ported from `web/index.html`'s `drawStage()`.
//! - [`fx`]: what every scene shares: note bursts (`| viz`), bass rings, vignette.
//!
//! Each frame the app calls [`Stage::update`] (sound, notes, section) and,
//! when the stage or its thumbnail is visible, [`Stage::paint`].

pub mod logic;
mod fx;
mod pen;
mod scenes;

use std::time::Instant;

use eframe::egui::{Align2, Color32, FontId, Painter, Pos2, Rect};
use loom_app::{Rgb, Theme};
use loom_audio::{Hit, Status};
use loom_core::Program;

use fx::Effects;
use logic::{Analyser, Director, Scene, VoiceLook};
use pen::{alpha, Pen};
use scenes::SceneState;

/// How long the scene name stays in the corner after a change, in seconds.
const NAME_SECONDS: f32 = 2.0;
/// Notes kept between two painted frames (the browser caps its queue at 400).
const MAX_PENDING_HITS: usize = 400;

/// The stage colours: the Omarchy theme where the browser uses its palette
/// (`--bg`, `--glow`, `--cyan`, `--amber`, `--dim`), made vivid, on a dark ground.
#[derive(Clone, Copy)]
pub struct Look {
    pub bg: Color32,
    pub glow: Color32,
    pub cyan: Color32,
    pub amber: Color32,
    pub dim: Color32,
    pub text: Color32,
}

impl Look {
    pub fn from_theme(theme: &Theme) -> Look {
        let vivid = |c: Rgb| {
            let (r, g, b) = logic::vivid(c.0, c.1, c.2);
            Color32::from_rgb(r, g, b)
        };
        let rgb = |c: Rgb| Color32::from_rgb(c.0, c.1, c.2);
        // a light theme keeps its colours, but the stage stays dark so light can glow
        let (bg, text) = if theme.dark {
            (theme.background, theme.foreground)
        } else {
            (theme.foreground.mix(Rgb(0, 0, 0), 0.6), theme.background)
        };
        Look { bg: rgb(bg), glow: vivid(theme.accent), cyan: vivid(theme.note), amber: vivid(theme.drum), dim: rgb(theme.dim.mix(text, 0.25)), text: rgb(text) }
    }
}

/// Text drawn over the full stage.
pub struct Overlay<'a> {
    /// Song title (`title:`), else the file name.
    pub title: &'a str,
    pub section: Option<&'a str>,
    /// Bar number to show (1-based), None when stopped.
    pub bar: Option<i64>,
    /// Pure-visual mode (T on the stage): draw the picture and no text at all.
    pub pure: bool,
}

/// Everything the stage remembers from frame to frame (buffers are reused).
pub struct Stage {
    pub director: Director,
    analyser: Analyser,
    voices: Vec<VoiceLook>,
    /// Per voice (same order as `voices`): a flash 0..1 when it plays a note.
    pulses: Vec<f32>,
    hits: Vec<Hit>,
    effects: Effects,
    /// Shake / flash / halo driven by the music, shared by every scene.
    post: fx::PostFx,
    state: SceneState,
    pen: Pen,
    last_paint: Option<Instant>,
    /// When the scene last changed (for the corner name).
    shown_at: Instant,
    shown: Scene,
    /// Les lignes `vis:` du morceau : quand il y en a, elles remplacent la scène.
    vis: loom_core::vis::Vis,
    vis_tex: Option<eframe::egui::TextureHandle>,
    vis_buf: Vec<u8>,
    /// Position du morceau (mesures) et temps par mesure, pour `beat` et `bar`.
    pos: f64,
    beats: f64,
    born: Instant,
}

impl Default for Stage {
    fn default() -> Self {
        let director = Director::default();
        Stage {
            shown: director.scene,
            director,
            analyser: Analyser::default(),
            voices: Vec::new(),
            pulses: Vec::new(),
            hits: Vec::new(),
            effects: Effects::default(),
            post: fx::PostFx::default(),
            state: SceneState::default(),
            pen: Pen::default(),
            last_paint: None,
            shown_at: Instant::now(),
            vis: Default::default(),
            vis_tex: None,
            vis_buf: Vec::new(),
            pos: 0.0,
            beats: 4.0,
            born: Instant::now(),
        }
    }
}

impl Stage {
    /// New code was RUN (or opened): read `scene:`, `scene@…:`, `title:` and the voices.
    pub fn load(&mut self, src: &str, program: &Program) {
        self.director.load(src);
        self.voices = logic::voice_looks(program);
        self.pulses = vec![0.0; self.voices.len()];
        self.vis = program.vis.clone();
        self.beats = program.meter_beats;
    }

    pub fn scene(&self) -> Scene {
        self.director.scene
    }

    /// N: next scene.
    pub fn next_scene(&mut self) {
        self.director.cycle();
    }

    /// V: VJ mode on/off.
    pub fn toggle_vj(&mut self) {
        self.director.vj = !self.director.vj;
    }

    /// Read the engine: output samples → spectrum, new notes, song position.
    /// `status` is None without a sound card.
    pub fn update(&mut self, status: Option<&Status>, program: &Program) {
        match status {
            Some(status) => {
                status.scope(&mut self.analyser.wave);
                let new_hits = status.take_hits();
                self.hits.extend(new_hits);
                if self.hits.len() > MAX_PENDING_HITS {
                    self.hits.drain(..self.hits.len() - MAX_PENDING_HITS);
                }
                self.pos = if status.playing() { status.position() } else { 0.0 };
                let playing = status.playing();
                let section = program.section_at(status.song_bar());
                self.director.follow(playing, status.bar(), section);
            }
            None => self.analyser.wave.fill(0.0),
        }
        self.analyser.process();
    }

    /// Draw the stage into `rect`. With an `overlay` it is the full stage
    /// (title, section, hints); without, a thumbnail.
    pub fn paint(&mut self, painter: &Painter, rect: Rect, look: &Look, overlay: Option<&Overlay>) {
        let now = Instant::now();
        // frames elapsed at 60 fps, so speeds match the browser at any frame rate
        let k = self.last_paint.map_or(1.0, |t| (now - t).as_secs_f32() * 60.0).clamp(0.0, 4.0);
        self.last_paint = Some(now);
        if self.director.scene != self.shown {
            self.shown = self.director.scene;
            self.shown_at = now;
        }

        // notes: light up their voice node
        for hit in &self.hits {
            if let Some(i) = self.voices.iter().position(|v| v.id == hit.voice) {
                self.pulses[i] = self.pulses[i].max(hit.gain.max(0.35));
            }
        }
        for p in &mut self.pulses {
            *p *= 0.9f32.powf(k);
        }

        let painter = painter.with_clip_rect(rect.intersect(painter.clip_rect()));
        self.pen.rect(rect, look.bg);
        // post-FX read the music first: the camera kicks NOW (the scene is drawn
        // into the displaced frame), the halo and the flash go on top after.
        self.post.update(k, self.analyser.levels.bass, self.analyser.levels.level);
        let scene_rect = self.post.shaken(rect);
        if !self.vis.is_empty() {
            // vis: l'image calculée par loom-core, petite et lissée, sous les éclats de notes
            self.pen.flush(&painter);
            self.paint_vis(&painter, scene_rect);
        }
        let mut frame = scenes::Frame::new(&mut self.pen, &painter, scene_rect, k, &self.analyser, look, &self.voices, &self.pulses, &self.hits);
        if self.vis.is_empty() { scenes::draw(self.director.scene, &mut self.state, &mut frame); }
        self.effects.draw(&mut frame, &self.voices);
        self.hits.clear();
        self.post.draw_bloom(&mut self.pen, rect, look);
        fx::vignette(&mut self.pen, rect, self.post.pump());
        self.post.draw_strobe(&mut self.pen, rect, look);
        self.pen.flush(&painter);

        match overlay {
            // pure-visual mode (T): the picture only, not one letter
            Some(overlay) if overlay.pure => {}
            Some(overlay) => self.paint_overlay(&painter, rect, look, overlay, now),
            None => {
                // thumbnail: just the scene's name
                let name = if self.vis.is_empty() { format!("scène : {}", self.director.scene.name()) } else { "scène : vis:".to_string() };
                painter.text(rect.left_bottom() + eframe::egui::vec2(6.0, -4.0), Align2::LEFT_BOTTOM, name, FontId::proportional(11.0), alpha(look.text, 0.8));
            }
        }
    }

    fn paint_vis(&mut self, painter: &Painter, rect: Rect) {
        use eframe::egui::{pos2, ColorImage, TextureOptions};
        let w = 160usize;
        let h = ((w as f32 * rect.height() / rect.width().max(1.0)).round() as usize).clamp(24, 160);
        let l = &self.analyser.levels;
        let pos = self.pos as f32;
        let inp = loom_core::vis::Inputs {
            t: self.born.elapsed().as_secs_f32(),
            low: l.bass,
            mid: l.mid,
            high: l.treble,
            level: l.level,
            beat: (pos * self.beats as f32).rem_euclid(1.0),
            bar: pos.floor(),
        };
        self.vis.render(w, h, &inp, &mut self.vis_buf);
        let img = ColorImage::from_rgba_unmultiplied([w, h], &self.vis_buf);
        match &mut self.vis_tex {
            Some(t) if t.size() == [w, h] => t.set(img, TextureOptions::LINEAR),
            _ => self.vis_tex = Some(painter.ctx().load_texture("loom-vis", img, TextureOptions::LINEAR)),
        }
        if let Some(t) = &self.vis_tex {
            painter.image(t.id(), rect, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE);
        }
    }

    fn paint_overlay(&self, painter: &Painter, rect: Rect, look: &Look, overlay: &Overlay, now: Instant) {
        let s = (rect.width().min(rect.height()) / 900.0).clamp(0.5, 2.0);
        let r = rect.width().min(rect.height()) * 0.2;
        let text = |pos: Pos2, align: Align2, t: &str, size: f32, color: Color32| {
            painter.text(pos, align, t, FontId::proportional(size), color);
        };

        // the section in the middle, like the browser (or the title, or LOOM)
        let middle = overlay.section.map(str::to_uppercase).unwrap_or_else(|| {
            if overlay.title.is_empty() { "LOOM".into() } else { overlay.title.to_uppercase() }
        });
        let size = (r * 0.2).max(14.0);
        text(rect.center() + eframe::egui::vec2(2.0, 2.0), Align2::CENTER_CENTER, &middle, size, alpha(Color32::BLACK, 0.35));
        text(rect.center(), Align2::CENTER_CENTER, &middle, size, alpha(look.glow, 0.6));

        // title, section and bar at the top left
        let corner = rect.left_top() + eframe::egui::vec2(24.0 * s, 20.0 * s);
        text(corner, Align2::LEFT_TOP, overlay.title, 26.0 * s, look.text);
        let mut position = match overlay.bar {
            Some(bar) => format!("mesure {bar}"),
            None => "arrêté".to_string(),
        };
        if let Some(section) = overlay.section {
            position = format!("{section} · {position}");
        }
        text(corner + eframe::egui::vec2(0.0, 34.0 * s), Align2::LEFT_TOP, &position, 17.0 * s, alpha(look.glow, 0.9));

        // the scene's name in the top right corner for 2 s after a change
        let age = (now - self.shown_at).as_secs_f32();
        if age < NAME_SECONDS {
            let fade = ((NAME_SECONDS - age) / 0.4).min(1.0);
            let pos = rect.right_top() + eframe::egui::vec2(-24.0 * s, 20.0 * s);
            text(pos, Align2::RIGHT_TOP, &format!("scène : {}", self.director.scene.name()), 30.0 * s, alpha(look.text, fade));
        }

        // how to drive it, at the bottom
        let vj = if self.director.vj { "VJ actif (V)" } else { "VJ (V)" };
        let hint = format!("scène : {} (N)  ·  {vj}  ·  texte (T)  ·  mesure {}  ·  Échap ou F11 pour revenir au code",
            self.director.scene.name(), overlay.bar.unwrap_or(0));
        text(Pos2::new(rect.center().x, rect.bottom() - 30.0 * s), Align2::CENTER_CENTER, &hint, 15.0 * s, look.dim);
    }
}
