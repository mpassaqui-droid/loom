//! The SCÈNE in the terminal (F12): a few of the browser's visuals drawn with
//! Braille dots (8 dots per character): spectrum bars, oscilloscope, a map of
//! the code's voices with note bursts, a starfield and a tunnel.
//!
//! The pure logic (scene choice, FFT, colours) is shared with `loom-gui`: the
//! file below is the GUI's own `scene/logic.rs`, compiled here too.

#[path = "../../loom-gui/src/scene/logic.rs"]
#[allow(dead_code)] // the GUI uses all of it, the terminal only part
pub mod logic;

use std::f64::consts::{PI, TAU};
use std::time::Instant;

use loom_app::Theme;
use loom_audio::Status;
use loom_core::Program;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::symbols::Marker;
use ratatui::text::{Line as TextLine, Span};
use ratatui::widgets::canvas::{Canvas, Circle, Line, Points};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

use logic::{hsl_to_rgb, Analyser, Director, Scene, VoiceLook, Viz};

/// How long the scene name shows after a change, in seconds.
const NAME_SECONDS: f32 = 2.0;

/// The scenes the terminal can draw.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TermScene {
    Bars,
    Scope,
    Codemap,
    Starfield,
    Tunnel,
}

impl TermScene {
    pub const ALL: [TermScene; 5] = [TermScene::Codemap, TermScene::Bars, TermScene::Scope, TermScene::Starfield, TermScene::Tunnel];

    /// The closest terminal scene for any scene of the code.
    pub fn from_scene(scene: Scene) -> TermScene {
        match scene {
            Scene::Bars | Scene::Spectro | Scene::Radial | Scene::Terrain => TermScene::Bars,
            Scene::Scope | Scene::Lissajous | Scene::Wave | Scene::Mandala | Scene::Lorenz | Scene::Dna => TermScene::Scope,
            Scene::Codemap | Scene::Fireworks | Scene::Network | Scene::Orbits | Scene::Metaballs => TermScene::Codemap,
            Scene::Starfield | Scene::Warp | Scene::Matrix | Scene::Fire => TermScene::Starfield,
            Scene::Tunnel | Scene::Spiral | Scene::Kaleido | Scene::Aurora | Scene::Plasma => TermScene::Tunnel,
        }
    }

    /// The scene of the code this one stands for (what N sets).
    pub fn scene(self) -> Scene {
        match self {
            TermScene::Bars => Scene::Bars,
            TermScene::Scope => Scene::Scope,
            TermScene::Codemap => Scene::Codemap,
            TermScene::Starfield => Scene::Starfield,
            TermScene::Tunnel => Scene::Tunnel,
        }
    }

    pub fn next(self) -> TermScene {
        let i = TermScene::ALL.iter().position(|s| *s == self).unwrap_or(0);
        TermScene::ALL[(i + 1) % TermScene::ALL.len()]
    }
}

/// A voice's hue as a terminal colour (HSL → RGB).
pub fn hue_color(hue: f32, lightness: f32) -> Color {
    let (r, g, b) = hsl_to_rgb(hue, 0.9, lightness);
    Color::Rgb(r, g, b)
}

/// Something to draw this frame, in canvas dots (x right, y up).
enum Mark {
    Line(f64, f64, f64, f64, Color),
    Dot(f64, f64, Color),
    Circle(f64, f64, f64, Color),
    Text(f64, f64, String, Color),
}

/// A note effect: a burst particle, or a ring / beam from `| viz`.
struct Spark {
    x: f64,
    y: f64,
    vx: f64,
    vy: f64,
    life: f64,
    hue: f32,
    kind: Viz,
}

/// The terminal stage: what it remembers between frames.
pub struct TermStage {
    pub director: Director,
    analyser: Analyser,
    voices: Vec<VoiceLook>,
    pulses: Vec<f32>,
    sparks: Vec<Spark>,
    stars: Vec<[f64; 3]>,
    marks: Vec<Mark>,
    time: f64,
    seed: u64,
    last: Instant,
    shown: TermScene,
    shown_at: Instant,
}

impl Default for TermStage {
    fn default() -> Self {
        TermStage {
            director: Director::default(),
            analyser: Analyser::default(),
            voices: Vec::new(),
            pulses: Vec::new(),
            sparks: Vec::new(),
            stars: Vec::new(),
            marks: Vec::new(),
            time: 0.0,
            seed: 0x9E37_79B9_7F4A_7C15,
            last: Instant::now(),
            shown: TermScene::Tunnel,
            shown_at: Instant::now(),
        }
    }
}

impl TermStage {
    /// New code was RUN or opened: its `scene:` lines, title and voices.
    pub fn load(&mut self, src: &str, program: &Program) {
        self.director.load(src);
        self.voices = logic::voice_looks(program);
        self.pulses = vec![0.0; self.voices.len()];
    }

    pub fn scene(&self) -> TermScene {
        TermScene::from_scene(self.director.scene)
    }

    /// N: next terminal scene.
    pub fn next_scene(&mut self) {
        self.director.scene = self.scene().next().scene();
    }

    fn random(&mut self) -> f64 {
        self.seed ^= self.seed << 13;
        self.seed ^= self.seed >> 7;
        self.seed ^= self.seed << 17;
        (self.seed >> 11) as f64 / (1u64 << 53) as f64
    }

    /// Read the engine: sound → spectrum, new notes → sparks, section → scene.
    pub fn update(&mut self, status: Option<&Status>, program: &Program) {
        let Some(status) = status else {
            self.analyser.wave.fill(0.0);
            self.analyser.process();
            return;
        };
        status.scope(&mut self.analyser.wave);
        self.analyser.process();
        self.director.follow(status.playing(), status.bar(), program.section_at(status.song_bar()));
        for hit in status.take_hits() {
            let look = self.voices.iter().position(|v| v.id == hit.voice);
            if let Some(i) = look {
                self.pulses[i] = self.pulses[i].max(hit.gain.max(0.35));
            }
            let kind = look.map_or(Viz::Burst, |i| self.voices[i].viz);
            if self.sparks.len() > 400 {
                continue;
            }
            match kind {
                Viz::Burst => {
                    let base = hit.hue as f64 / 360.0 * TAU;
                    for _ in 0..4 + (hit.gain * 8.0) as usize {
                        let a = base + (self.random() - 0.5) * 1.1;
                        let speed = 0.012 + hit.gain as f64 * 0.02 * (0.5 + self.random());
                        self.sparks.push(Spark { x: 0.0, y: 0.0, vx: a.cos() * speed, vy: a.sin() * speed, life: 1.0, hue: hit.hue, kind });
                    }
                }
                _ => self.sparks.push(Spark { x: 0.0, y: 0.0, vx: 0.0, vy: 0.0, life: 1.0, hue: hit.hue, kind }),
            }
        }
    }

    /// Draw the full-screen stage.
    pub fn draw(&mut self, f: &mut Frame, area: Rect, theme: &Theme, title: &str, position: &str) {
        let now = Instant::now();
        let k = ((now - self.last).as_secs_f64() * 60.0).clamp(0.0, 6.0);
        self.last = now;
        let scene = self.scene();
        if scene != self.shown {
            self.shown = scene;
            self.shown_at = now;
        }
        let rgb = |c: loom_app::Rgb| Color::Rgb(c.0, c.1, c.2);
        let (bg, accent, note, dim) = (rgb(theme.background), rgb(theme.accent), rgb(theme.note), rgb(theme.dim));

        // canvas in dots: 2 across and 4 down per character, so circles are round
        let (w, h) = (area.width as f64 * 2.0, area.height.saturating_sub(2) as f64 * 4.0);
        let (cx, cy) = (w / 2.0, h / 2.0);
        let size = w.min(h);
        self.marks.clear();
        let levels = self.analyser.levels;
        let bass = levels.bass as f64;
        for p in &mut self.pulses {
            *p *= 0.9f32.powf(k as f32);
        }

        match scene {
            TermScene::Bars => {
                let bars = (w / 3.0) as usize;
                let bins = self.analyser.spectrum.len() as f64 * 0.6;
                for i in 0..bars {
                    let fraction = i as f64 / bars as f64;
                    let m = self.analyser.spectrum[(fraction * fraction * bins) as usize] as f64;
                    let top = m.powf(1.4) * h * 0.9;
                    if top < 1.0 {
                        continue;
                    }
                    let x = i as f64 * 3.0 + 1.0;
                    let color = hue_color((220.0 - fraction * 220.0) as f32, 0.5 + m as f32 * 0.2);
                    self.marks.push(Mark::Line(x, 0.0, x, top, color));
                    self.marks.push(Mark::Line(x + 1.0, 0.0, x + 1.0, top, color));
                }
                self.waveform(w, cy, h * 0.18, rgb(theme.drum), false);
            }
            TermScene::Scope => {
                for gx in 0..=10 {
                    let x = w * gx as f64 / 10.0;
                    for gy in 0..=16 {
                        self.marks.push(Mark::Dot(x, h * gy as f64 / 16.0, dim));
                    }
                }
                self.marks.push(Mark::Line(0.0, cy, w, cy, dim));
                self.waveform(w, cy, h * 0.4, accent, true);
            }
            TermScene::Codemap => {
                let n = self.voices.len().max(1) as f64;
                let radius = size * 0.32;
                self.marks.push(Mark::Circle(cx, cy, 2.0 + bass * 8.0, note));
                for (i, v) in self.voices.iter().enumerate() {
                    let ang = i as f64 / n * TAU + PI / 2.0 - self.time * 0.04;
                    let band = if v.drum { levels.bass } else if i % 2 == 1 { levels.treble } else { levels.mid };
                    let e = band.max(self.pulses[i]) as f64;
                    let (x, y) = (cx + ang.cos() * radius, cy + ang.sin() * radius);
                    let color = hue_color(v.hue, 0.45 + e as f32 * 0.3);
                    self.marks.push(Mark::Line(cx, cy, x, y, hue_color(v.hue, 0.25 + e as f32 * 0.25)));
                    let rr = 2.0 + (v.gain as f64 * 3.0).min(1.0) * 3.0 + e * 7.0;
                    let mut r = rr;
                    while r > 0.5 {
                        self.marks.push(Mark::Circle(x, y, r, color));
                        r -= 1.5;
                    }
                    self.marks.push(Mark::Text(x - v.name.chars().count() as f64, y + rr + 3.0, v.name.clone(), hue_color(v.hue, 0.8)));
                }
                if self.voices.is_empty() {
                    self.marks.push(Mark::Text(cx - 30.0, cy - 12.0, "écris des voix puis Ctrl+R".into(), dim));
                }
            }
            TermScene::Starfield => {
                if self.stars.is_empty() {
                    for _ in 0..220 {
                        let star = [self.random() * 2.0 - 1.0, self.random() * 2.0 - 1.0, self.random() * 0.9 + 0.1];
                        self.stars.push(star);
                    }
                }
                let speed = (0.004 + bass * 0.035) * k;
                for i in 0..self.stars.len() {
                    let mut star = self.stars[i];
                    star[2] -= speed;
                    if star[2] <= 0.02 {
                        star = [self.random() * 2.0 - 1.0, self.random() * 2.0 - 1.0, 1.0];
                    }
                    self.stars[i] = star;
                    let z = 0.5 / star[2];
                    let (x, y) = (cx + star[0] * z * w, cy + star[1] * z * h);
                    let color = if star[2] < 0.4 { accent } else { note };
                    self.marks.push(Mark::Dot(x, y, color));
                    if star[2] < 0.25 {
                        self.marks.push(Mark::Dot(x + 1.0, y, color));
                    }
                }
            }
            TermScene::Tunnel => {
                const N: usize = 12;
                let max_r = (w * w + h * h).sqrt() / 2.0;
                for ring in 0..N {
                    let phase = (self.time * 0.8 + ring as f64 / N as f64).fract();
                    let hue = (ring as f64 * 30.0 + self.time * 25.0) as f32;
                    let color = hue_color(hue, 0.25 + (1.0 - phase as f32) * 0.4);
                    self.marks.push(Mark::Circle(cx, cy, phase * phase * max_r * 1.05, color));
                }
            }
        }
        self.time += (0.01 + bass * 0.05) * k;

        // note effects, over every scene
        let reach = size * 0.5;
        for s in &mut self.sparks {
            s.life -= 0.02 * k;
            if s.life <= 0.0 {
                continue;
            }
            match s.kind {
                Viz::Burst => {
                    s.x += s.vx * k;
                    s.y += s.vy * k;
                    s.vx *= 0.97f64.powf(k);
                    s.vy *= 0.97f64.powf(k);
                    self.marks.push(Mark::Dot(cx + s.x * reach * 2.0, cy - s.y * reach * 2.0, hue_color(s.hue, 0.35 + s.life as f32 * 0.3)));
                }
                Viz::Ring | Viz::Flash => self.marks.push(Mark::Circle(cx, cy, (1.0 - s.life) * reach * if s.kind == Viz::Flash { 2.0 } else { 1.0 }, hue_color(s.hue, 0.55))),
                Viz::Beam => {
                    let a = s.hue as f64 / 360.0 * TAU;
                    self.marks.push(Mark::Line(cx, cy, cx + a.cos() * reach * 2.0, cy - a.sin() * reach * 2.0, hue_color(s.hue, 0.3 + s.life as f32 * 0.35)));
                }
                Viz::Shape => {
                    let sides = 3 + (s.hue / 72.0) as usize;
                    let r = (1.0 - s.life) * reach + 4.0;
                    for i in 0..sides {
                        let (a1, a2) = (i as f64 / sides as f64 * TAU, (i + 1) as f64 / sides as f64 * TAU);
                        self.marks.push(Mark::Line(cx + a1.cos() * r, cy + a1.sin() * r, cx + a2.cos() * r, cy + a2.sin() * r, hue_color(s.hue, 0.55)));
                    }
                }
            }
        }
        self.sparks.retain(|s| s.life > 0.0);

        // title and position on top, the scene and keys at the bottom
        let top = Rect { height: 1, ..area };
        let bottom = Rect { y: area.bottom().saturating_sub(1), height: 1, ..area };
        let canvas_area = Rect { y: area.y + 1, height: area.height.saturating_sub(2), ..area };
        let bold = Style::new().fg(accent).add_modifier(Modifier::BOLD);
        let head = TextLine::from(vec![
            Span::styled(format!(" {} ", if title.is_empty() { "LOOM" } else { title }), bold),
            Span::styled(format!(" {position}"), Style::new().fg(rgb(theme.foreground))),
        ]);
        f.render_widget(Paragraph::new(head).style(Style::new().bg(bg)), top);

        let marks = &self.marks;
        let canvas = Canvas::default()
            .marker(Marker::Braille)
            .background_color(bg)
            .x_bounds([0.0, w.max(1.0)])
            .y_bounds([0.0, h.max(1.0)])
            .paint(|ctx| {
                for mark in marks {
                    match mark {
                        Mark::Line(x1, y1, x2, y2, c) => ctx.draw(&Line::new(*x1, *y1, *x2, *y2, *c)),
                        Mark::Dot(x, y, c) => ctx.draw(&Points::new(&[(*x, *y)], *c)),
                        Mark::Circle(x, y, r, c) => ctx.draw(&Circle::new(*x, *y, *r, *c)),
                        Mark::Text(x, y, t, c) => ctx.print(*x, *y, TextLine::styled(t.clone(), Style::new().fg(*c))),
                    }
                }
            });
        f.render_widget(canvas, canvas_area);

        // the code's scene name, and what the terminal draws for it when different
        let name = match self.director.scene.name() {
            code if code == scene.scene().name() => code.to_string(),
            code => format!("{code} → {}", scene.scene().name()),
        };
        let age = (now - self.shown_at).as_secs_f32();
        if age < NAME_SECONDS {
            let name = format!(" scène : {name} ");
            let width = name.chars().count() as u16;
            let corner = Rect { x: area.right().saturating_sub(width + 1), y: area.y + 1, width: width.min(area.width), height: 1 };
            f.render_widget(Paragraph::new(Span::styled(name, Style::new().fg(bg).bg(accent).add_modifier(Modifier::BOLD))), corner);
        }
        let vj = if self.director.vj { "VJ actif" } else { "VJ" };
        let hint = TextLine::from(vec![
            Span::styled(format!(" scène : {name} "), bold),
            Span::styled(format!(" N scène suivante · V {vj} · Échap/F12 retour au code · Ctrl+R jouer · F6 stop"), Style::new().fg(dim)),
        ]);
        f.render_widget(Paragraph::new(hint).style(Style::new().bg(bg)), bottom);
    }

    /// The waveform across the canvas (`trigger`: start on a rising zero crossing).
    fn waveform(&mut self, w: f64, y0: f64, amp: f64, color: Color, trigger: bool) {
        let wave = &self.analyser.wave;
        let span = wave.len() / 2;
        let start = if trigger { (1..span).find(|&i| wave[i - 1] < 0.0 && wave[i] >= 0.0).unwrap_or(0) } else { 0 };
        let points = (w as usize).max(2);
        let mut last = (0.0, y0 + wave[start] as f64 * amp);
        for p in 1..points {
            let i = start + p * span / points;
            let point = (p as f64, y0 + wave[i] as f64 * amp);
            self.marks.push(Mark::Line(last.0, last.1, point.0, point.1, color));
            last = point;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_scene_has_a_terminal_version() {
        for scene in Scene::ALL {
            let term = TermScene::from_scene(scene);
            assert_eq!(TermScene::from_scene(term.scene()), term);
        }
    }

    #[test]
    fn n_cycles_through_the_terminal_scenes() {
        let mut stage = TermStage::default();
        stage.director.scene = Scene::Plasma; // shown as the tunnel
        assert_eq!(stage.scene(), TermScene::Tunnel);
        let mut seen = Vec::new();
        for _ in 0..TermScene::ALL.len() {
            stage.next_scene();
            seen.push(stage.scene());
        }
        assert_eq!(stage.scene(), TermScene::Tunnel, "wraps around");
        assert!(TermScene::ALL.iter().all(|s| seen.contains(s)));
    }

    #[test]
    fn hues_become_terminal_colours() {
        assert_eq!(hue_color(0.0, 0.5), Color::Rgb(242, 13, 13));
        assert_eq!(hue_color(240.0, 0.5), Color::Rgb(13, 13, 242));
    }
}
