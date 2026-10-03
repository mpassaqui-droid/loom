//! The SCÈNE in the terminal (F12): the 24 native scenes of the window app,
//! drawn with Braille dots (8 dots per character), so `scene: lorenz` in the
//! code shows a Lorenz attractor here too.
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
    shown: Scene,
    shown_at: Instant,
    /// Fire embers and firework sparks: x, y, vx, vy, life, hue (canvas dots).
    particles: Vec<[f64; 6]>,
    /// Past spectra, newest first (spectrogram and terrain).
    history: std::collections::VecDeque<Vec<f32>>,
    /// The Lorenz attractor: its state and the trail drawn.
    lorenz: [f64; 3],
    trail: std::collections::VecDeque<(f64, f64)>,
    /// Matrix rain: head row and speed per column.
    rain: Vec<(f64, f64)>,
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
            shown: Scene::Tunnel,
            shown_at: Instant::now(),
            particles: Vec::new(),
            history: std::collections::VecDeque::new(),
            lorenz: [1.0, 1.0, 20.0],
            trail: std::collections::VecDeque::new(),
            rain: Vec::new(),
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

    pub fn scene(&self) -> Scene {
        self.director.scene
    }

    /// N: next scene.
    pub fn next_scene(&mut self) {
        self.director.scene = self.director.scene.next();
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
            if self.director.scene == Scene::Fireworks && self.particles.len() < 900 {
                let (x, y) = (self.random(), 0.35 + self.random() * 0.55);
                for _ in 0..10 + (hit.gain * 20.0) as usize {
                    let a = self.random() * TAU;
                    let v = 0.004 + self.random() * 0.01 * (0.5 + hit.gain as f64);
                    self.particles.push([x, y, a.cos() * v, a.sin() * v, 1.0, hit.hue as f64]);
                }
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
            self.particles.clear();
            self.trail.clear();
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

        let (mid, treble) = (levels.mid as f64, levels.treble as f64);
        let t = self.time;
        let bins = self.analyser.spectrum.len() as f64 * 0.6;
        // spectre lu en échelle quasi logarithmique : f in 0..1
        let spec = |sp: &[f32], f: f64| sp[((f * f * bins) as usize).min(sp.len().saturating_sub(1))] as f64;
        if matches!(scene, Scene::Spectro | Scene::Terrain) {
            let cols = (w / 2.0).max(1.0) as usize;
            let row: Vec<f32> = (0..cols).map(|i| spec(&self.analyser.spectrum, i as f64 / cols as f64) as f32).collect();
            self.history.push_front(row);
            self.history.truncate(64);
        }

        match scene {
            Scene::Bars => {
                let bars = (w / 3.0) as usize;
                for i in 0..bars {
                    let fraction = i as f64 / bars as f64;
                    let m = spec(&self.analyser.spectrum, fraction);
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
            Scene::Radial => {
                const RAYS: usize = 96;
                let r0 = size * (0.12 + bass * 0.05);
                for i in 0..RAYS {
                    let f = (i as f64 / RAYS as f64 * 2.0 - 1.0).abs();
                    let m = spec(&self.analyser.spectrum, f);
                    let a = i as f64 / RAYS as f64 * TAU + t * 0.1;
                    let r1 = r0 + 1.0 + m.powf(1.2) * size * 0.4;
                    self.marks.push(Mark::Line(cx + a.cos() * r0, cy + a.sin() * r0, cx + a.cos() * r1, cy + a.sin() * r1, hue_color((f * 300.0) as f32, 0.45 + m as f32 * 0.25)));
                }
                self.marks.push(Mark::Circle(cx, cy, r0 * 0.9, note));
            }
            Scene::Scope => {
                for gx in 0..=10 {
                    let x = w * gx as f64 / 10.0;
                    for gy in 0..=16 {
                        self.marks.push(Mark::Dot(x, h * gy as f64 / 16.0, dim));
                    }
                }
                self.marks.push(Mark::Line(0.0, cy, w, cy, dim));
                self.waveform(w, cy, h * 0.4, accent, true);
            }
            Scene::Wave => {
                for j in 0..6 {
                    let band = [bass, mid, treble][j % 3];
                    let y0 = h * (0.15 + j as f64 * 0.14);
                    let amp = h * 0.04 * (0.4 + band * 2.0);
                    let f = 0.02 + j as f64 * 0.008;
                    let color = hue_color((j * 50) as f32 + t as f32 * 10.0, 0.55);
                    let mut last = (0.0, y0);
                    let mut x = 0.0;
                    while x <= w {
                        let p = (x, y0 + (x * f + t * (1.0 + j as f64 * 0.3)).sin() * amp);
                        if x > 0.0 { self.marks.push(Mark::Line(last.0, last.1, p.0, p.1, color)); }
                        last = p;
                        x += 2.0;
                    }
                }
            }
            Scene::Lissajous => {
                let wave = &self.analyser.wave;
                let n = wave.len();
                let lag = n / 4;
                let r = size * 0.45;
                let mut last: Option<(f64, f64)> = None;
                for i in (0..n - lag).step_by(2) {
                    let p = (cx + wave[i] as f64 * r * 1.6, cy + wave[i + lag] as f64 * r * 1.6);
                    if let Some(l) = last { self.marks.push(Mark::Line(l.0, l.1, p.0, p.1, hue_color(i as f32 / n as f32 * 300.0, 0.55))); }
                    last = Some(p);
                }
            }
            Scene::Kaleido => {
                let wave = &self.analyser.wave;
                let pts = 48usize;
                let span = wave.len() / 2;
                for k6 in 0..6 {
                    let rot = k6 as f64 * TAU / 6.0 + t * 0.15;
                    for mirror in [1.0, -1.0] {
                        let mut last: Option<(f64, f64)> = None;
                        for p in 0..pts {
                            let r = size * 0.05 + p as f64 / pts as f64 * size * 0.45;
                            let d = wave[p * span / pts] as f64 * size * 0.25 * mirror + mirror * (p as f64 * 0.3 + t).sin() * 3.0;
                            let a = rot + d / r.max(1.0);
                            let q = (cx + a.cos() * r, cy + a.sin() * r);
                            if let Some(l) = last { self.marks.push(Mark::Line(l.0, l.1, q.0, q.1, hue_color((k6 * 60) as f32 + p as f32 * 3.0, 0.55))); }
                            last = Some(q);
                        }
                    }
                }
            }
            Scene::Spiral => {
                let n = 360;
                for i in 0..n {
                    let f = i as f64 / n as f64;
                    for arm in 0..3 {
                        let a = f * TAU * 3.0 + t * 0.6 + arm as f64 * TAU / 3.0;
                        let r = f * size * 0.5 * (1.0 + bass * 0.3);
                        self.marks.push(Mark::Dot(cx + a.cos() * r, cy + a.sin() * r, hue_color((f * 360.0 + arm as f64 * 120.0) as f32, 0.35 + f as f32 * 0.3)));
                    }
                }
            }
            Scene::Tunnel => {
                const N: usize = 12;
                let max_r = (w * w + h * h).sqrt() / 2.0;
                for ring in 0..N {
                    let phase = (t * 0.8 + ring as f64 / N as f64).fract();
                    let hue = (ring as f64 * 30.0 + t * 25.0) as f32;
                    let color = hue_color(hue, 0.25 + (1.0 - phase as f32) * 0.4);
                    self.marks.push(Mark::Circle(cx, cy, phase * phase * max_r * 1.05, color));
                }
            }
            Scene::Starfield | Scene::Warp => {
                if self.stars.is_empty() {
                    for _ in 0..220 {
                        let star = [self.random() * 2.0 - 1.0, self.random() * 2.0 - 1.0, self.random() * 0.9 + 0.1];
                        self.stars.push(star);
                    }
                }
                let warp = scene == Scene::Warp;
                let speed = (if warp { 0.012 } else { 0.004 } + bass * 0.035) * k;
                for i in 0..self.stars.len() {
                    let mut star = self.stars[i];
                    let z0 = star[2];
                    star[2] -= speed;
                    if star[2] <= 0.02 {
                        star = [self.random() * 2.0 - 1.0, self.random() * 2.0 - 1.0, 1.0];
                    }
                    self.stars[i] = star;
                    let z = 0.5 / star[2];
                    let (x, y) = (cx + star[0] * z * w, cy + star[1] * z * h);
                    let color = if star[2] < 0.4 { accent } else { note };
                    if warp && z0 > star[2] {
                        let zt = 0.5 / (z0 + speed * 3.0);
                        self.marks.push(Mark::Line(cx + star[0] * zt * w, cy + star[1] * zt * h, x, y, hue_color(200.0 + star[0] as f32 * 80.0, 0.3 + (1.0 - star[2] as f32) * 0.4)));
                    } else {
                        self.marks.push(Mark::Dot(x, y, color));
                        if star[2] < 0.25 {
                            self.marks.push(Mark::Dot(x + 1.0, y, color));
                        }
                    }
                }
            }
            Scene::Codemap | Scene::Network => {
                let n = self.voices.len().max(1) as f64;
                let radius = size * 0.32;
                let pos: Vec<(f64, f64, f64)> = self.voices.iter().enumerate().map(|(i, v)| {
                    let ang = i as f64 / n * TAU + PI / 2.0 - t * 0.04;
                    let band = if v.drum { levels.bass } else if i % 2 == 1 { levels.treble } else { levels.mid };
                    let e = band.max(self.pulses[i]) as f64;
                    let wob = if scene == Scene::Network { (t * 0.7 + i as f64).sin() * size * 0.06 } else { 0.0 };
                    (cx + ang.cos() * (radius + wob), cy + ang.sin() * (radius + wob), e)
                }).collect();
                if scene == Scene::Network {
                    for i in 0..pos.len() {
                        for j in i + 1..pos.len() {
                            let e = (pos[i].2 + pos[j].2) / 2.0;
                            self.marks.push(Mark::Line(pos[i].0, pos[i].1, pos[j].0, pos[j].1, hue_color((self.voices[i].hue + self.voices[j].hue) / 2.0, 0.15 + e as f32 * 0.4)));
                        }
                    }
                } else {
                    self.marks.push(Mark::Circle(cx, cy, 2.0 + bass * 8.0, note));
                }
                for (i, v) in self.voices.iter().enumerate() {
                    let (x, y, e) = pos[i];
                    let color = hue_color(v.hue, 0.45 + e as f32 * 0.3);
                    if scene == Scene::Codemap {
                        self.marks.push(Mark::Line(cx, cy, x, y, hue_color(v.hue, 0.25 + e as f32 * 0.25)));
                    }
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
            Scene::Orbits => {
                self.marks.push(Mark::Circle(cx, cy, 3.0 + bass * 6.0, accent));
                let n = self.voices.len().max(1);
                for i in 0..n {
                    let r = size * (0.1 + 0.38 * (i + 1) as f64 / n as f64);
                    self.marks.push(Mark::Circle(cx, cy, r, dim));
                    let (hue, e) = self.voices.get(i).map_or((200.0, 0.0), |v| (v.hue, self.pulses[i] as f64));
                    let a = t * (0.9 / (i + 1) as f64) + i as f64 * 1.7;
                    let (x, y) = (cx + a.cos() * r, cy + a.sin() * r);
                    let mut rr = 2.0 + e * 6.0;
                    while rr > 0.5 {
                        self.marks.push(Mark::Circle(x, y, rr, hue_color(hue, 0.5 + e as f32 * 0.3)));
                        rr -= 1.5;
                    }
                }
            }
            Scene::Metaballs => {
                let n = self.voices.len().max(3);
                for i in 0..n {
                    let (hue, e) = self.voices.get(i).map_or((i as f32 * 90.0, bass as f32), |v| (v.hue, self.pulses[i]));
                    let x = cx + (t * (0.3 + i as f64 * 0.11) + i as f64).sin() * w * 0.3;
                    let y = cy + (t * (0.4 + i as f64 * 0.07) + i as f64 * 2.0).cos() * h * 0.3;
                    let big = size * (0.06 + e as f64 * 0.08 + bass * 0.04);
                    let mut r = big;
                    while r > 0.5 {
                        self.marks.push(Mark::Circle(x, y, r, hue_color(hue, 0.25 + (1.0 - (r / big) as f32) * 0.45)));
                        r -= 1.2;
                    }
                }
            }
            Scene::Plasma => {
                let mut y = 0.0;
                while y < h {
                    let mut x = 0.0;
                    while x < w {
                        let v = (x * 0.05 + t).sin() + (y * 0.07 - t * 1.3).sin() + ((x + y) * 0.03 + t * 0.5).sin() + ((x - cx).hypot(y - cy) * 0.06 - t).sin();
                        let hue = (v * 60.0 + t * 30.0).rem_euclid(360.0) as f32;
                        let c = hue_color(hue, 0.35 + (v as f32 + 4.0) / 8.0 * 0.3 + bass as f32 * 0.1);
                        self.marks.push(Mark::Dot(x, y, c));
                        if v > 0.0 { self.marks.push(Mark::Dot(x + 1.0, y + 2.0, c)); }
                        x += 2.0;
                    }
                    y += 4.0;
                }
            }
            Scene::Aurora => {
                for c in 0..4 {
                    let base = h * (0.45 + c as f64 * 0.1);
                    let mut x = 0.0;
                    while x <= w {
                        let y = base + (x * 0.015 + t * (0.6 + c as f64 * 0.2) + c as f64).sin() * h * 0.08 * (1.0 + mid);
                        let len = h * (0.12 + 0.1 * (x * 0.03 - t + c as f64).sin().abs()) * (0.6 + bass);
                        let hue = 120.0 + c as f32 * 40.0 + (x * 0.3) as f32;
                        self.marks.push(Mark::Line(x, y, x, y - len, hue_color(hue, 0.25 + c as f32 * 0.07)));
                        x += 3.0;
                    }
                }
            }
            Scene::Fire | Scene::Fireworks => {
                let fire = scene == Scene::Fire;
                if fire {
                    let spawn = (4.0 + bass * 30.0) * k;
                    for _ in 0..spawn as usize {
                        if self.particles.len() > 900 { break; }
                        let x = 0.15 + self.random() * 0.7;
                        let vy = 0.004 + self.random() * 0.008 * (1.0 + mid * 2.0);
                        let vx = (self.random() - 0.5) * 0.002;
                        self.particles.push([x, 0.0, vx, vy, 1.0, 0.0]);
                    }
                } else if self.particles.len() < 40 {
                    let (x, y) = (self.random(), 0.4 + self.random() * 0.5);
                    let hue = self.random() * 360.0;
                    for _ in 0..60 {
                        let a = self.random() * TAU;
                        let v = 0.002 + self.random() * 0.004;
                        self.particles.push([x, y, a.cos() * v, a.sin() * v, 1.0, hue]);
                    }
                }
                for p in &mut self.particles {
                    p[0] += p[2] * k;
                    p[1] += p[3] * k;
                    p[4] -= if fire { 0.012 } else { 0.01 } * k;
                    if !fire { p[3] -= 0.00012 * k; }
                    let hue = if fire { (p[4] * 50.0) as f32 } else { p[5] as f32 };
                    let light = if fire { 0.2 + p[4] as f32 * 0.4 } else { 0.3 + p[4] as f32 * 0.35 };
                    self.marks.push(Mark::Dot(p[0] * w, p[1] * h, hue_color(hue, light)));
                }
                self.particles.retain(|p| p[4] > 0.0 && p[1] >= -0.05);
            }
            Scene::Matrix => {
                let cols = (w / 4.0).max(1.0) as usize;
                if self.rain.len() != cols {
                    self.rain = (0..cols).map(|_| (0.0, 0.0)).collect();
                    for i in 0..cols { self.rain[i] = (self.random() * h, 0.6 + self.random() * 1.6); }
                }
                const GLYPHS: &[u8] = b"01LOOM<>/|*+=x.";
                for i in 0..cols {
                    let (head, speed) = self.rain[i];
                    let mut head = head - speed * (1.0 + bass * 3.0) * k;
                    if head < -h * 0.4 { head = h + self.random() * h * 0.3; }
                    self.rain[i] = (head, speed);
                    for j in 0..8 {
                        let y = head + j as f64 * 4.0;
                        if y < 0.0 || y > h { continue; }
                        let g = GLYPHS[(i * 7 + j * 3 + (t * 4.0) as usize) % GLYPHS.len()] as char;
                        let c = if j == 0 { rgb(theme.foreground) } else { hue_color(120.0, 0.55 - j as f32 * 0.05) };
                        self.marks.push(Mark::Text(i as f64 * 4.0, y, g.to_string(), c));
                    }
                }
            }
            Scene::Lorenz => {
                let (sigma, rho, beta) = (10.0, 28.0, 8.0 / 3.0);
                // une trace déjà dessinée dès l'arrivée sur la scène
                let steps = if self.trail.is_empty() { 700 } else { (3.0 * k).ceil() as usize + (bass * 6.0) as usize };
                for _ in 0..steps {
                    let [x, y, z] = self.lorenz;
                    let dt = 0.006;
                    self.lorenz = [x + sigma * (y - x) * dt, y + (x * (rho - z) - y) * dt, z + (x * y - beta * z) * dt];
                    let s = size / 60.0;
                    self.trail.push_front((cx + self.lorenz[0] * s * 1.3, cy + (self.lorenz[2] - 25.0) * s));
                }
                self.trail.truncate(900);
                let n = self.trail.len();
                for (i, pair) in self.trail.iter().zip(self.trail.iter().skip(1)).enumerate() {
                    let f = 1.0 - i as f32 / n as f32;
                    self.marks.push(Mark::Line(pair.0 .0, pair.0 .1, pair.1 .0, pair.1 .1, hue_color(200.0 + f * 140.0, 0.15 + f * 0.45)));
                }
            }
            Scene::Spectro | Scene::Terrain => {
                if scene == Scene::Spectro {
                    for (r, row) in self.history.iter().enumerate() {
                        let y = h - r as f64 * 4.0;
                        if y < 0.0 { break; }
                        for (c, m) in row.iter().enumerate() {
                            if *m > 0.04 {
                                self.marks.push(Mark::Dot(c as f64 * 2.0, y, hue_color(240.0 - *m * 240.0, 0.25 + *m * 0.4)));
                            }
                        }
                    }
                } else {
                    for (r, row) in self.history.iter().enumerate().take(20).collect::<Vec<_>>().into_iter().rev() {
                        let depth = r as f64 / 20.0;
                        let y0 = h * (0.08 + depth * 0.55);
                        let scale = 1.0 - depth * 0.5;
                        let x0 = cx - w * 0.5 * scale;
                        let color = hue_color(160.0 + depth as f32 * 120.0, 0.55 - depth as f32 * 0.3);
                        let mut last: Option<(f64, f64)> = None;
                        for (c, m) in row.iter().enumerate().step_by(2) {
                            let p = (x0 + c as f64 * 2.0 * scale, y0 + *m as f64 * h * 0.3 * scale);
                            if let Some(l) = last { self.marks.push(Mark::Line(l.0, l.1, p.0, p.1, color)); }
                            last = Some(p);
                        }
                    }
                }
            }
            Scene::Dna => {
                let amp = h * 0.25 * (1.0 + bass * 0.5);
                let mut last: Option<((f64, f64), (f64, f64))> = None;
                let mut x = 0.0;
                let mut i = 0;
                while x <= w {
                    let a = x * 0.04 + t * 2.0;
                    let (p1, p2) = ((x, cy + a.sin() * amp), (x, cy - a.sin() * amp));
                    if let Some((l1, l2)) = last {
                        self.marks.push(Mark::Line(l1.0, l1.1, p1.0, p1.1, accent));
                        self.marks.push(Mark::Line(l2.0, l2.1, p2.0, p2.1, note));
                    }
                    if i % 6 == 0 {
                        self.marks.push(Mark::Line(p1.0, p1.1, p2.0, p2.1, hue_color((x * 2.0) as f32 + t as f32 * 20.0, 0.4 + mid as f32 * 0.3)));
                    }
                    last = Some((p1, p2));
                    x += 2.0;
                    i += 1;
                }
            }
            Scene::Mandala => {
                for j in 0..6 {
                    let sides = 3 + j;
                    let band = [bass, mid, treble][j % 3];
                    let r = size * (0.07 + j as f64 * 0.07) * (1.0 + band * 0.4);
                    let rot = t * 0.3 * if j % 2 == 0 { 1.0 } else { -1.0 };
                    let color = hue_color((j * 55) as f32 + t as f32 * 15.0, 0.5);
                    for s2 in 0..sides {
                        let (a1, a2) = (rot + s2 as f64 / sides as f64 * TAU, rot + (s2 + 1) as f64 / sides as f64 * TAU);
                        let (x1, y1) = (cx + a1.cos() * r, cy + a1.sin() * r);
                        self.marks.push(Mark::Line(x1, y1, cx + a2.cos() * r, cy + a2.sin() * r, color));
                        self.marks.push(Mark::Circle(x1, y1, 1.5 + band * 4.0, color));
                    }
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

        let name = scene.name().to_string();
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
    fn n_cycles_through_every_scene() {
        let mut stage = TermStage::default();
        let mut seen = Vec::new();
        for _ in 0..Scene::ALL.len() {
            stage.next_scene();
            seen.push(stage.scene());
        }
        assert!(Scene::ALL.iter().all(|s| seen.contains(s)), "24 scènes");
    }

    #[test]
    fn every_scene_draws_something() {
        use ratatui::backend::TestBackend;
        let mut term = ratatui::Terminal::new(TestBackend::new(80, 30)).unwrap();
        let theme = Theme::default();
        let program = Program::parse("kick: x . x .\nlead: c4 e4 g4");
        for scene in Scene::ALL {
            let mut stage = TermStage::default();
            stage.load("kick: x . x .\nlead: c4 e4 g4", &program);
            stage.director.scene = scene;
            // un signal pour que les scènes qui suivent le son aient de quoi dessiner
            for (i, v) in stage.analyser.wave.iter_mut().enumerate() { *v = (i as f32 * 0.05).sin() * 0.6; }
            stage.analyser.process();
            for _ in 0..8 {
                stage.last = Instant::now() - std::time::Duration::from_millis(16);
                term.draw(|f| stage.draw(f, f.area(), &theme, "t", "")).unwrap();
            }
            let buf = term.backend().buffer().clone();
            let inked = buf.content().iter().skip(80).take(80 * 28).filter(|c| !c.symbol().trim().is_empty() && c.symbol() != "\u{2800}").count();
            assert!(inked > 20, "{} ne dessine presque rien ({inked} cases)", scene.name());
        }
    }

    #[test]
    fn hues_become_terminal_colours() {
        assert_eq!(hue_color(0.0, 0.5), Color::Rgb(242, 13, 13));
        assert_eq!(hue_color(240.0, 0.5), Color::Rgb(13, 13, 242));
    }
}
