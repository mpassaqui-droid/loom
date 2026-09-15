//! The scenes, ported from `drawStage()` in `web/index.html`.
//!
//! The browser draws each frame in canvas 2D; here the same maths fills the
//! [`Pen`]. Numbers are kept from the JS (`sceneTime += 0.008 + bass*0.05`…),
//! with sizes in pixels multiplied by the frame scale `s` and speeds by `k`
//! (frames elapsed at 60 fps). `spectrum` is the browser's `fb[i]/255` and
//! `wave` its `(tb[i]-128)/128`.

use std::f32::consts::{PI, TAU};

use eframe::egui::{Align2, Color32, FontId, Painter, Pos2, Rect, Vec2};
use loom_audio::Hit;

use super::fx::{Particle, Rng};
use super::logic::{hsv_to_rgb, Analyser, Scene, VoiceLook};
use super::pen::{alpha, glow, hsl, Pen};
use super::Look;

/// Everything a scene needs to draw one frame.
pub struct Frame<'a> {
    pub pen: &'a mut Pen,
    pub painter: &'a Painter,
    pub rect: Rect,
    pub w: f32,
    pub h: f32,
    pub c: Pos2,
    /// Pixel scale: 1 on a ~900 px high stage.
    pub s: f32,
    /// Frames elapsed since the last paint, at 60 fps.
    pub k: f32,
    pub audio: &'a Analyser,
    pub look: &'a Look,
    pub voices: &'a [VoiceLook],
    pub pulses: &'a [f32],
    /// Notes started since the last frame.
    pub hits: &'a [Hit],
}

impl<'a> Frame<'a> {
    #[allow(clippy::too_many_arguments)]
    pub fn new(pen: &'a mut Pen, painter: &'a Painter, rect: Rect, k: f32, audio: &'a Analyser, look: &'a Look,
               voices: &'a [VoiceLook], pulses: &'a [f32], hits: &'a [Hit]) -> Frame<'a> {
        let (w, h) = (rect.width().max(1.0), rect.height().max(1.0));
        Frame { pen, painter, rect, w, h, c: rect.center(), s: (w.min(h) / 900.0).clamp(0.1, 3.0), k, audio, look, voices, pulses, hits }
    }

    fn bass(&self) -> f32 {
        self.audio.levels.bass
    }

    /// Spectrum bin `i` (0..1), clamped to the available bins.
    fn fb(&self, i: usize) -> f32 {
        self.audio.spectrum[i.min(self.audio.spectrum.len() - 1)]
    }

    fn pos(&self, x: f32, y: f32) -> Pos2 {
        Pos2::new(self.rect.left() + x, self.rect.top() + y)
    }

    /// Draw text over what the pen drew so far.
    fn text(&mut self, pos: Pos2, align: Align2, text: &str, size: f32, color: Color32) {
        self.pen.flush(self.painter);
        self.painter.text(pos, align, text, FontId::monospace(size), color);
    }
}

/// What the scenes remember between frames.
#[derive(Default)]
pub struct SceneState {
    time: f32,
    rng: Rng,
    /// Scratch buffer for polylines, reused every frame.
    points: Vec<Pos2>,
    stars: Vec<[f32; 3]>,
    net: Vec<[f32; 4]>,
    fire: Vec<Particle>,
    sparks: Vec<Particle>,
    drops: Vec<f32>,
    warp: Vec<[f32; 2]>,
    spectro: Spectrogram,
}

/// Draw one frame of `scene`.
pub fn draw(scene: Scene, st: &mut SceneState, f: &mut Frame) {
    match scene {
        Scene::Codemap => codemap(st, f),
        Scene::Radial => radial(st, f),
        Scene::Tunnel => tunnel(st, f),
        Scene::Bars => bars(st, f),
        Scene::Starfield => starfield(st, f),
        Scene::Kaleido => kaleido(st, f),
        Scene::Terrain => terrain(st, f),
        Scene::Spiral => spiral(st, f),
        Scene::Aurora => aurora(st, f),
        Scene::Metaballs => metaballs(st, f),
        Scene::Network => network(st, f),
        Scene::Lissajous => lissajous(st, f),
        Scene::Plasma => plasma(st, f),
        Scene::Fire => fire(st, f),
        Scene::Matrix => matrix(st, f),
        Scene::Orbits => orbits(st, f),
        Scene::Wave => wave(st, f),
        Scene::Lorenz => lorenz(st, f),
        Scene::Warp => warp(st, f),
        Scene::Spectro => spectro(st, f),
        Scene::Scope => scope(st, f),
        Scene::Dna => dna(st, f),
        Scene::Mandala => mandala(st, f),
        Scene::Fireworks => fireworks(st, f),
    }
}

/// Glowing rings flying out of the centre, faster with the bass.
fn tunnel(st: &mut SceneState, f: &mut Frame) {
    let bass = f.bass();
    st.time += (0.008 + bass * 0.05) * f.k;
    let max_r = (f.w * f.w + f.h * f.h).sqrt() / 2.0;
    const N: usize = 16;
    for k in 0..N {
        let ph = (st.time + k as f32 / N as f32).fract();
        let color = if k % 2 == 1 { f.look.glow } else { f.look.cyan };
        f.pen.glow_ring(f.c, ph * max_r * 1.05, (2.0 + bass * 10.0) * f.s, color, (1.0 - ph) * 0.6);
    }
}

/// Mirrored spectrum bars with a waveform across.
fn bars(st: &mut SceneState, f: &mut Frame) {
    const NB: usize = 72;
    let step = (f.audio.spectrum.len() * 6 / 10 / NB).max(1);
    let bw = (f.w / 2.0) / NB as f32;
    let (cyan, green, amber) = (f.look.cyan, f.look.glow, f.look.amber);
    for i in 0..NB {
        let h = f.fb(i * step).powf(1.4) * f.h * 0.46;
        if h < 0.5 {
            continue;
        }
        for x in [f.c.x + i as f32 * bw, f.c.x - (i + 1) as f32 * bw] {
            let (left, right) = (x + 1.0, x + bw - 1.0);
            let (bottom, mid, top) = (f.rect.bottom(), f.rect.bottom() - h * 0.6, f.rect.bottom() - h);
            f.pen.quad([Pos2::new(left, mid), Pos2::new(right, mid), Pos2::new(right, bottom), Pos2::new(left, bottom)], [green, green, cyan, cyan]);
            f.pen.quad([Pos2::new(left, top), Pos2::new(right, top), Pos2::new(right, mid), Pos2::new(left, mid)], [amber, amber, green, green]);
            f.pen.vgradient(Rect::from_min_max(Pos2::new(left - bw * 0.4, top), Pos2::new(right + bw * 0.4, bottom)), glow(amber, 0.12), glow(cyan, 0.2));
        }
    }
    st.points.clear();
    let wave = &f.audio.wave;
    for i in (0..wave.len()).step_by(4) {
        st.points.push(f.pos(i as f32 / wave.len() as f32 * f.w, f.h / 2.0 + wave[i] * f.h * 0.18));
    }
    f.pen.glow_polyline(&st.points, 2.0 * f.s.max(0.5), amber, 1.0);
}

/// One node per voice of the code, pulsing when it plays.
fn codemap(st: &mut SceneState, f: &mut Frame) {
    let levels = f.audio.levels;
    let bass = levels.bass;
    let s = f.s;
    f.pen.glow_dot(f.c, (12.0 + bass * 34.0) * s, f.look.cyan, 0.22 + bass * 0.5);
    if f.voices.is_empty() {
        let pos = f.c + Vec2::new(0.0, 80.0 * s);
        f.text(pos, Align2::CENTER_CENTER, "écris des voix (kick: x . x .) : chacune devient un nœud", 16.0 * s, f.look.dim);
        st.time += 0.01 * f.k;
        return;
    }
    let n = f.voices.len() as f32;
    let radius = f.w.min(f.h) * 0.30;
    let mut labels: Vec<(Pos2, f32)> = Vec::with_capacity(f.voices.len());
    for (i, v) in f.voices.iter().enumerate() {
        let ang = i as f32 / n * TAU - PI / 2.0 + st.time * 0.04;
        let band = if v.drum { bass } else if i % 2 == 1 { levels.treble } else { levels.mid };
        let e = band.max(f.pulses.get(i).copied().unwrap_or(0.0));
        let at = f.c + Vec2::angled(ang) * radius;
        let rr = (12.0 + (v.gain * 3.0).min(1.0) * 22.0 + e * 34.0) * s;
        f.pen.line(f.c, at, (1.0 + e * 4.0) * s, alpha(hsl(v.hue, 0.7, 0.55), 0.12 + e * 0.55));
        f.pen.radial(at, rr + (8.0 + e * 24.0) * s, glow(hsl(v.hue, 0.9, 0.6), 0.35 + e * 0.4), Color32::TRANSPARENT);
        f.pen.disc(at, rr, alpha(hsl(v.hue, 0.85, 0.42 + e * 0.32), 0.9));
        labels.push((at - Vec2::new(0.0, rr + 12.0 * s), v.hue));
    }
    for (i, (pos, hue)) in labels.into_iter().enumerate() {
        let name = f.voices[i].name.as_str();
        f.pen.flush(f.painter);
        f.painter.text(pos, Align2::CENTER_CENTER, name, FontId::monospace((16.0 * s).max(8.0)), hsl(hue, 0.55, 0.86));
    }
    st.time += 0.01 * f.k;
}

/// A retro grid landscape rolling towards you, hills from the spectrum.
fn terrain(st: &mut SceneState, f: &mut Frame) {
    let bass = f.bass();
    let (w, h, s) = (f.w, f.h, f.s);
    let hz = h * 0.5;
    st.time += (0.02 + bass * 0.06) * f.k;
    for i in -12..=12 {
        let i = i as f32;
        f.pen.glow_line(f.pos(w / 2.0 + i * w * 0.06, h), f.pos(w / 2.0 + i * w * 0.008, hz), s.max(0.5), f.look.cyan, 0.35);
    }
    for r in 0..16 {
        let t2 = ((r as f32 + st.time) % 16.0) / 16.0;
        let y = hz + (h - hz) * t2 * t2;
        let amp = f.fb(r * 3) * 34.0 * (1.0 + bass) * s;
        st.points.clear();
        let mut x = 0.0;
        while x <= w + 18.0 * s {
            let yy = y - ((x / s) * 0.012 + st.time * 3.0).sin() * amp * t2;
            st.points.push(f.pos(x, yy));
            x += 18.0 * s;
        }
        f.pen.glow_polyline(&st.points, s.max(0.6), f.look.glow, 0.55 * (1.0 - t2) + 0.15);
    }
    let band = 46.0 * s;
    let amber = alpha(f.look.amber, 0.45);
    f.pen.vgradient(Rect::from_min_max(f.pos(0.0, hz - band), f.pos(w, hz)), Color32::TRANSPARENT, amber);
    f.pen.vgradient(Rect::from_min_max(f.pos(0.0, hz), f.pos(w, hz + band)), amber, Color32::TRANSPARENT);
}

/// Three spiral arms of dots, sized by the spectrum.
fn spiral(st: &mut SceneState, f: &mut Frame) {
    let bass = f.bass();
    st.time += (0.01 + bass * 0.05) * f.k;
    const ARMS: usize = 3;
    const PTS: usize = 170;
    let max_r = f.w.min(f.h) * 0.46;
    let colors = [f.look.glow, f.look.cyan, f.look.amber];
    let half = f.audio.spectrum.len() as f32 * 0.5;
    for (a, color) in colors.iter().enumerate().take(ARMS) {
        let off = a as f32 / ARMS as f32 * TAU;
        for i in 2..PTS {
            let t2 = i as f32 / PTS as f32;
            let ang = off + t2 * 11.0 + st.time * (1.0 + a as f32 * 0.12);
            let m = f.fb((t2 * half) as usize);
            let rr = max_r * t2.powf(0.72) * (0.92 + bass * 0.22);
            let size = (1.0 + m * 5.0 + t2 * 1.4) * f.s;
            f.pen.glow_dot(f.c + Vec2::angled(ang) * rr, size, *color, 0.22 + m * 0.72);
        }
    }
}

/// Curtains of light swaying with the music.
fn aurora(st: &mut SceneState, f: &mut Frame) {
    let bass = f.bass();
    st.time += (0.01 + bass * 0.04) * f.k;
    const BANDS: usize = 6;
    let (w, h, s, t) = (f.w, f.h, f.s, st.time);
    let step = 18.0 * s.max(0.3);
    for b in 0..BANDS {
        let m = f.fb(b * 6 + 2);
        let color = [f.look.glow, f.look.cyan, f.look.amber][b % 3];
        let bx = w * (0.12 + 0.76 * (b as f32 / (BANDS - 1) as f32));
        let wd = w * 0.05 * (0.55 + m * 1.2);
        let bf = b as f32;
        let sway = |y: f32| ((y / s) * 0.006 + t * 1.5 + bf * 1.3).sin() * w * 0.06 * (0.5 + m) + ((y / s) * 0.013 - t * 2.0 + bf).sin() * w * 0.03;
        // brightest 40 % down, fading to nothing at both ends
        let strength = |y: f32| {
            let u = y / h;
            let peak = 0.05 + m * 0.3;
            (if u < 0.4 { u / 0.4 } else { (1.0 - u) / 0.6 }) * peak * 1.8
        };
        let mut y = 0.0;
        while y < h {
            let y2 = (y + step).min(h);
            let (x1, x2) = (bx + sway(y), bx + sway(y2));
            let (c1, c2) = (glow(color, strength(y)), glow(color, strength(y2)));
            f.pen.quad([f.pos(x1 - wd, y), f.pos(x1 + wd, y), f.pos(x2 + wd, y2), f.pos(x2 - wd, y2)], [c1, c1, c2, c2]);
            y = y2;
        }
    }
}

/// Soft coloured blobs orbiting and swelling with the spectrum.
fn metaballs(st: &mut SceneState, f: &mut Frame) {
    let bass = f.bass();
    st.time += (0.008 + bass * 0.03) * f.k;
    let size = f.w.min(f.h);
    let t = st.time;
    for b in 0..7 {
        let bf = b as f32;
        let m = f.fb(b * 5 + 3);
        let color = [f.look.glow, f.look.cyan, f.look.amber][b % 3];
        let ang = t * (0.6 + bf * 0.12) + bf * 1.7;
        let rad = size * (0.12 + 0.28 * (t * 0.7 + bf).sin());
        let at = f.c + Vec2::new(ang.cos() * rad * 1.4, (ang * 0.9).sin() * rad);
        let r3 = size * (0.07 + 0.13 * m);
        let middle = glow(color, 0.12 * 1.5);
        f.pen.radial(at, r3 * 0.5, glow(color, (0.55 + m * 0.4) * 1.2), middle);
        f.pen.band(at, r3 * 0.5, r3, middle, Color32::TRANSPARENT);
    }
}

/// Drifting points joined when they come close.
fn network(st: &mut SceneState, f: &mut Frame) {
    st.time += 0.01 * f.k;
    if st.net.is_empty() {
        for _ in 0..64 {
            let p = [st.rng.next(), st.rng.next(), st.rng.centered() * 0.6, st.rng.centered() * 0.6];
            st.net.push(p);
        }
    }
    let bass = f.bass();
    let (w, h, s) = (f.w, f.h, f.s);
    let speed = (1.0 + bass * 3.5) * f.k * s;
    for p in &mut st.net {
        p[0] += p[2] * speed / w;
        p[1] += p[3] * speed / h;
        if !(0.0..=1.0).contains(&p[0]) { p[2] = -p[2]; }
        if !(0.0..=1.0).contains(&p[1]) { p[3] = -p[3]; }
        p[0] = p[0].clamp(0.0, 1.0);
        p[1] = p[1].clamp(0.0, 1.0);
    }
    let md = w.min(h) * 0.19;
    for i in 0..st.net.len() {
        for j in i + 1..st.net.len() {
            let (a, b) = (f.pos(st.net[i][0] * w, st.net[i][1] * h), f.pos(st.net[j][0] * w, st.net[j][1] * h));
            let d = a.distance(b);
            if d < md {
                f.pen.line(a, b, s.max(0.5), alpha(f.look.cyan, (1.0 - d / md) * 0.45));
            }
        }
    }
    for p in &st.net {
        f.pen.glow_dot(f.pos(p[0] * w, p[1] * h), (2.0 + bass * 2.0) * s, f.look.glow, 1.0);
    }
}

/// The waveform against itself, slightly delayed: an oscilloscope figure.
fn lissajous(st: &mut SceneState, f: &mut Frame) {
    let wave = &f.audio.wave;
    let n = wave.len();
    let r4 = f.w.min(f.h) * 0.42;
    let lag = (n as f32 * 0.012) as usize + 1;
    st.points.clear();
    for i in (0..n - lag).step_by(2) {
        st.points.push(f.c + Vec2::new(wave[i], wave[i + lag]) * r4);
    }
    f.pen.glow_polyline(&st.points, 2.2 * f.s.max(0.4), f.look.glow, 1.0);
    st.points.clear();
    for i in (0..n - lag).step_by(3) {
        st.points.push(f.c + Vec2::new(wave[i + lag], wave[i]) * r4);
    }
    f.pen.polyline(&st.points, f.s.max(0.4), alpha(f.look.cyan, 0.5));
}

/// Rainbow interference of sine waves (the browser's 160×90 pixel plasma, here a smooth colour mesh).
fn plasma(st: &mut SceneState, f: &mut Frame) {
    let bass = f.bass();
    st.time += (0.014 + bass * 0.05) * f.k;
    const GX: usize = 64;
    const GY: usize = 36;
    let t = st.time;
    let color_at = |px: f32, py: f32| {
        let v = (px * 0.06 + t).sin() + (py * 0.08 - t * 0.7).sin() + ((px + py) * 0.05 + t * 1.3).sin()
            + (((px - 80.0).powi(2) + (py - 45.0).powi(2)).sqrt() * 0.07 - t * 1.6).sin();
        let (r, g, b) = hsv_to_rgb(v * 55.0 + t * 40.0, 0.7, (0.45 + 0.4 * (v * 1.5).sin()) * (1.0 + bass * 0.5));
        Color32::from_rgba_unmultiplied(r, g, b, 230)
    };
    let (cw, ch) = (f.w / GX as f32, f.h / GY as f32);
    for gy in 0..GY {
        for gx in 0..GX {
            let corner = |x: usize, y: usize| (f.pos(x as f32 * cw, y as f32 * ch), color_at(x as f32 * 160.0 / GX as f32, y as f32 * 90.0 / GY as f32));
            let (a, b, c, d) = (corner(gx, gy), corner(gx + 1, gy), corner(gx + 1, gy + 1), corner(gx, gy + 1));
            f.pen.quad([a.0, b.0, c.0, d.0], [a.1, b.1, c.1, d.1]);
        }
    }
}

/// Embers rising from the bottom, more with the bass.
fn fire(st: &mut SceneState, f: &mut Frame) {
    st.time += 0.02 * f.k;
    let bass = f.bass();
    let (dw, dh) = (f.w / f.s, f.h / f.s);
    // more room than the browser's 760: without its fading trails, flames need more embers
    let emit = ((8.0 + bass * 30.0) * f.k.min(2.0)) as usize;
    for _ in 0..emit {
        if st.fire.len() >= 1600 {
            break;
        }
        let vy = -(1.5 + st.rng.next() * 2.5 + bass * 4.5);
        st.fire.push(Particle { pos: Vec2::new(st.rng.next() * dw, dh + 5.0), vel: Vec2::new(st.rng.centered() * 1.2, vy), life: 1.0, color: None });
    }
    let t = st.time;
    let damping = 0.99f32.powf(f.k);
    for p in &mut st.fire {
        p.pos.x += (p.vel.x + ((p.pos.y + t * 40.0) * 0.05).sin() * 0.6) * f.k;
        p.pos.y += p.vel.y * f.k;
        p.vel.y *= damping;
        p.life -= 0.018 * f.k;
        if p.life <= 0.0 || p.pos.y < -10.0 {
            p.life = 0.0;
            continue;
        }
        let lf = p.life;
        let color = Color32::from_rgb(255, (lf * 230.0) as u8, ((lf - 0.62).max(0.0) * 255.0) as u8);
        let (at, r) = (f.pos(p.pos.x * f.s, p.pos.y * f.s), ((1.0 - lf) * 6.0 + 3.0) * f.s);
        f.pen.radial(at, r * 2.2, glow(color, lf * 0.7), Color32::TRANSPARENT);
        f.pen.disc(at, r * 0.5, glow(color, lf * 0.6));
    }
    st.fire.retain(|p| p.life > 0.0);
}

/// Digital rain. The browser leaves fading trails by never fully clearing
/// its canvas; here each drop draws its own fading tail.
fn matrix(st: &mut SceneState, f: &mut Frame) {
    const GLYPHS: &[u8] = b"0123456789ABCDEF@#$%&*+=<>LOOMloom";
    const TAIL: usize = 10;
    let size = (18.0 * f.s).max(8.0);
    let cols = ((f.w / size) as usize).max(1);
    if st.drops.len() != cols {
        st.drops.clear();
        for _ in 0..cols {
            let d = st.rng.next() * (f.h / size + TAIL as f32) - TAIL as f32;
            st.drops.push(d);
        }
    }
    st.time += f.k;
    let bass = f.bass();
    // the glowing heads go under the text
    for i in 0..cols {
        let y = st.drops[i] * size;
        if y > 0.0 && y < f.h + size {
            f.pen.radial(f.pos(i as f32 * size + size * 0.3, y - size * 0.4), size * 0.9, glow(f.look.glow, 0.25), Color32::TRANSPARENT);
        }
    }
    f.pen.flush(f.painter);
    let font = FontId::monospace(size);
    let flicker = (st.time / 6.0) as usize; // glyphs change a few times a second
    let mut buf = [0u8; 4];
    for i in 0..cols {
        let head = st.drops[i].floor() as i64;
        for j in 0..TAIL {
            let row = head - j as i64;
            let y = row as f32 * size + size;
            if y < 0.0 || y - size > f.h {
                continue;
            }
            let pick = (i * 31 + row.unsigned_abs() as usize * 17 + if j < 2 { flicker } else { 0 }) % GLYPHS.len();
            let glyph = (GLYPHS[pick] as char).encode_utf8(&mut buf);
            let color = if j == 0 { alpha(f.look.glow, 0.95) } else { alpha(f.look.cyan, 0.5 * (1.0 - j as f32 / TAIL as f32)) };
            f.painter.text(f.pos(i as f32 * size, y), Align2::LEFT_BOTTOM, &*glyph, font.clone(), color);
        }
        st.drops[i] += (0.4 + bass * 1.4) * f.k;
        if (head - TAIL as i64) as f32 * size > f.h && st.rng.next() > 0.975 {
            st.drops[i] = st.rng.next() * -18.0;
        }
    }
}

/// Rings of moons turning in opposite directions.
fn orbits(st: &mut SceneState, f: &mut Frame) {
    let bass = f.bass();
    st.time += (0.01 + bass * 0.03) * f.k;
    let size = f.w.min(f.h);
    for r in 0..6 {
        let rf = r as f32;
        let rad = size * (0.07 + 0.066 * rf);
        let count = 6 + r * 3;
        let spin = st.time * (0.5 + rf * 0.28) * if r % 2 == 1 { 1.0 } else { -1.0 };
        let m = f.fb(r * 6);
        let color = [f.look.glow, f.look.cyan, f.look.amber][r % 3];
        f.pen.ring(f.c, rad, f.s.max(0.5), alpha(color, 0.1));
        for i in 0..count {
            let a = i as f32 / count as f32 * TAU + spin;
            f.pen.glow_dot(f.c + Vec2::angled(a) * rad, (2.0 + m * 4.0) * f.s, color, 0.4 + m * 0.6);
        }
    }
    f.pen.glow_dot(f.c, (4.0 + bass * 8.0) * f.s, f.look.glow, 0.5 + bass * 0.5);
}

/// Stacked waveforms, like a mountain range.
fn wave(st: &mut SceneState, f: &mut Frame) {
    const LAYERS: usize = 18;
    let size = f.w.min(f.h);
    let wave = &f.audio.wave;
    for l in (0..=LAYERS).rev() {
        let t2 = l as f32 / LAYERS as f32;
        let y0 = f.c.y - size * 0.32 + t2 * size * 0.64;
        let color = if l % 2 == 1 { f.look.glow } else { f.look.cyan };
        let fade = 1.0 - t2;
        st.points.clear();
        for i in (0..wave.len()).step_by(8) {
            let x = f.rect.left() + i as f32 / wave.len() as f32 * f.w;
            st.points.push(Pos2::new(x, y0 - wave[i] * size * 0.13 * (0.35 + fade)));
        }
        f.pen.glow_polyline(&st.points, 1.5 * f.s.max(0.4), color, 0.12 + fade * 0.5);
    }
}

/// The Lorenz "butterfly" attractor, slowly turning.
fn lorenz(st: &mut SceneState, f: &mut Frame) {
    let bass = f.bass();
    st.time += 0.005 * f.k;
    let (mut x, mut y, mut z) = (0.1f32, 0.0f32, 0.0f32);
    let dt = 0.006;
    let scale = f.w.min(f.h) * 0.013 * (1.0 + bass * 0.15);
    let (ca, sa) = ((st.time * 0.5).cos(), (st.time * 0.5).sin());
    st.points.clear();
    for _ in 0..2600 {
        let (dx, dy, dz) = (10.0 * (y - x), x * (28.0 - z) - y, x * y - 2.667 * z);
        x += dx * dt;
        y += dy * dt;
        z += dz * dt;
        st.points.push(f.c + Vec2::new((x * ca - y * sa) * scale, (z - 25.0) * scale));
    }
    f.pen.glow_polyline(&st.points, (1.0 + bass * 1.5) * f.s.max(0.5), f.look.glow, 0.5);
}

/// Hyperspace streaks.
fn warp(st: &mut SceneState, f: &mut Frame) {
    st.time += 0.01 * f.k;
    let max_r = (f.w * f.w + f.h * f.h).sqrt() * 0.5 / f.s;
    if st.warp.is_empty() {
        for _ in 0..200 {
            let star = [st.rng.next() * TAU, st.rng.next() * max_r];
            st.warp.push(star);
        }
    }
    let speed = (4.0 + f.bass() * 34.0) * f.k;
    for star in &mut st.warp {
        let (a, r) = (star[0], star[1]);
        let r2 = r + speed;
        let t2 = (r / max_r).min(1.0);
        let color = if t2 > 0.5 { f.look.glow } else { f.look.cyan };
        let dir = Vec2::angled(a) * f.s;
        f.pen.glow_line(f.c + dir * r, f.c + dir * r2, (0.6 + t2 * 3.0) * f.s, color, t2);
        star[1] = r2;
        if r2 > max_r {
            star[0] = st.rng.next() * TAU;
            star[1] = st.rng.next() * 30.0;
        }
    }
}

/// The spectrogram's history: a ring buffer of coloured columns.
pub struct Spectrogram {
    colors: Vec<Color32>,
    head: usize,
    /// Time since the last column, in frames.
    wait: f32,
}

impl Spectrogram {
    const COLS: usize = 240;
    const ROWS: usize = 96;
}

impl Default for Spectrogram {
    fn default() -> Self {
        Spectrogram { colors: vec![Color32::TRANSPARENT; Self::COLS * Self::ROWS], head: 0, wait: 0.0 }
    }
}

/// Scrolling spectrogram: time to the left, high notes up.
fn spectro(st: &mut SceneState, f: &mut Frame) {
    st.time += 0.02 * f.k;
    let (cols, rows) = (Spectrogram::COLS, Spectrogram::ROWS);
    let sg = &mut st.spectro;
    sg.wait += f.k;
    // one new column every 2 frames: 8 seconds of history
    while sg.wait >= 2.0 {
        sg.wait -= 2.0;
        let bins = f.audio.spectrum.len() as f32;
        for y in 0..rows {
            let fr = 1.0 - y as f32 / rows as f32;
            let mg = f.fb((fr * fr * bins * 0.72) as usize);
            sg.colors[sg.head * rows + y] = if mg > 0.02 {
                hsl((250.0 - mg * 250.0 + st.time * 8.0).rem_euclid(360.0), 0.92, 0.18 + mg * 0.48)
            } else {
                Color32::TRANSPARENT
            };
        }
        sg.head = (sg.head + 1) % cols;
    }
    let (cw, ch) = (f.w / cols as f32, f.h / rows as f32);
    for i in 0..cols {
        let col = (sg.head + i) % cols; // oldest on the left
        for y in 0..rows {
            let color = sg.colors[col * rows + y];
            if color != Color32::TRANSPARENT {
                f.pen.rect(Rect::from_min_size(f.pos(i as f32 * cw, y as f32 * ch), Vec2::new(cw + 0.5, ch + 0.5)), color);
            }
        }
    }
    let pos = f.pos(8.0, f.h - 10.0 * f.s.max(0.6));
    f.text(pos, Align2::LEFT_BOTTOM, "spectrogramme · aigu en haut · le temps défile vers la gauche", (11.0 * f.s).max(8.0), alpha(f.look.dim, 0.9));
}

/// Analog phosphor oscilloscope.
fn scope(st: &mut SceneState, f: &mut Frame) {
    let dim = f.look.dim;
    let line = f.s.max(0.5);
    for gx in 0..=10 {
        let x = f.w * gx as f32 / 10.0;
        f.pen.line(f.pos(x, 0.0), f.pos(x, f.h), line, alpha(dim, 0.35));
    }
    for gy in 0..=8 {
        let y = f.h * gy as f32 / 8.0;
        f.pen.line(f.pos(0.0, y), f.pos(f.w, y), line, alpha(dim, 0.35));
    }
    f.pen.line(f.pos(0.0, f.h / 2.0), f.pos(f.w, f.h / 2.0), line, alpha(dim, 0.65));
    f.pen.line(f.pos(f.w / 2.0, 0.0), f.pos(f.w / 2.0, f.h), line, alpha(dim, 0.65));

    // trigger on a rising zero crossing so the trace stands still
    let wave = &f.audio.wave;
    let span = wave.len() / 2;
    let start = (1..span).find(|&i| wave[i - 1] < 0.0 && wave[i] >= 0.0).unwrap_or(0);
    let amp = f.h * 0.34;
    st.points.clear();
    for i in (0..span).step_by(2) {
        st.points.push(f.pos(f.w * i as f32 / span as f32, f.h / 2.0 + wave[start + i] * amp));
    }
    f.pen.polyline(&st.points, 6.0 * f.s.max(0.4), glow(f.look.glow, 0.35));
    f.pen.glow_polyline(&st.points, 2.5 * f.s.max(0.4), f.look.glow, 1.0);
    let pos = f.pos(10.0, 18.0 * f.s.max(0.6));
    f.text(pos, Align2::LEFT_CENTER, "oscilloscope", (11.0 * f.s).max(8.0), alpha(dim, 0.9));
}

/// A twin helix, its rungs lit by the spectrum.
fn dna(st: &mut SceneState, f: &mut Frame) {
    let bass = f.bass();
    st.time += (0.03 + bass * 0.08) * f.k;
    let amp = f.w * 0.16 * (0.6 + bass * 0.6);
    const TURNS: f32 = 3.0;
    let step = 6.0 * f.s.max(0.3);
    for strand in 0..2 {
        st.points.clear();
        let mut y = 0.0;
        while y <= f.h {
            let ph = y / f.h * TURNS * TAU + st.time + strand as f32 * PI;
            st.points.push(f.pos(f.w / 2.0 + ph.sin() * amp, y));
            y += step;
        }
        let color = if strand == 1 { f.look.cyan } else { f.look.glow };
        f.pen.glow_polyline(&st.points, 2.0 * f.s.max(0.4), color, 0.7);
    }
    let bins = f.audio.spectrum.len() as f32;
    let mut y = 0.0;
    while y < f.h {
        let ph = y / f.h * TURNS * TAU + st.time;
        let m = f.fb((y / f.h * bins * 0.4) as usize);
        let color = hsl(((y / f.s) * 0.5 + st.time * 30.0).rem_euclid(360.0), 0.88, 0.62);
        f.pen.line(f.pos(f.w / 2.0 + ph.sin() * amp, y), f.pos(f.w / 2.0 - ph.sin() * amp, y), (1.0 + m * 4.0) * f.s.max(0.4), alpha(color, 0.4 + m * 0.5));
        y += 22.0 * f.s.max(0.3);
    }
}

/// Twelve rotated copies of the waveform, in a turning rainbow.
fn mandala(st: &mut SceneState, f: &mut Frame) {
    let bass = f.bass();
    st.time += (0.006 + bass * 0.02) * f.k;
    const SEGMENTS: usize = 12;
    let max_r = f.w.min(f.h) * 0.46;
    let wave = &f.audio.wave;
    for seg in 0..SEGMENTS {
        let turn = seg as f32 / SEGMENTS as f32 * TAU + st.time;
        let color = hsl((seg as f32 / SEGMENTS as f32 * 360.0 + st.time * 50.0).rem_euclid(360.0), 0.88, 0.62);
        st.points.clear();
        for i in (0..wave.len()).step_by(8) {
            let r = max_r * (0.12 + wave[i].abs() * 0.5);
            let a = i as f32 / wave.len() as f32 * 1.2 + turn;
            st.points.push(f.c + Vec2::angled(a) * r);
        }
        f.pen.glow_polyline(&st.points, 2.0 * f.s.max(0.4), color, 0.6);
    }
}

/// Rainbow sparks around the centre from the spectrum, and an explosion for every note.
fn fireworks(st: &mut SceneState, f: &mut Frame) {
    let bass = f.bass();
    st.time += 0.016 * f.k;
    const N: usize = 90;
    let size = f.w.min(f.h);
    let half = f.audio.spectrum.len() as f32 * 0.5;
    for i in 0..N {
        let a = i as f32 / N as f32 * TAU + st.time * 0.2;
        let m = f.fb((i as f32 * half / N as f32) as usize).powf(1.5);
        let r = size * 0.08 + m * size * 0.46 * (0.5 + bass);
        let at = f.c + Vec2::angled(a) * r;
        let color = hsl((i as f32 / N as f32 * 360.0 + st.time * 60.0).rem_euclid(360.0), 0.88, 0.62);
        let side = (2.0 + m * 7.0) * f.s;
        if m > 0.05 {
            f.pen.radial(at, side * 2.0, glow(color, m * 0.6), Color32::TRANSPARENT);
        }
        f.pen.rect(Rect::from_center_size(at, Vec2::splat(side)), alpha(color, 0.4 + m * 0.6));
    }

    // one shell per note, somewhere on screen, in the voice's colour
    let (dw, dh) = (f.w / f.s, f.h / f.s);
    for hit in f.hits {
        if st.sparks.len() > 900 {
            break;
        }
        let center = Vec2::new(dw * (0.15 + 0.7 * st.rng.next()), dh * (0.12 + 0.5 * st.rng.next()));
        let color = hsl(hit.hue, 0.9, 0.62);
        let count = 18 + (hit.gain * 30.0) as usize;
        for _ in 0..count {
            let v = Vec2::angled(st.rng.next() * TAU) * (1.5 + st.rng.next() * 4.0) * (0.6 + hit.gain);
            st.sparks.push(Particle { pos: center, vel: v, life: 1.0, color: Some(color) });
        }
    }
    let damping = 0.965f32.powf(f.k);
    for p in &mut st.sparks {
        p.pos += p.vel * f.k;
        p.vel = p.vel * damping + Vec2::new(0.0, 0.06 * f.k);
        p.life -= 0.013 * f.k;
        if p.life > 0.0 {
            let color = p.color.unwrap_or(f.look.glow);
            f.pen.glow_dot(f.pos(p.pos.x * f.s, p.pos.y * f.s), (1.2 + p.life * 2.0) * f.s, color, p.life);
        }
    }
    st.sparks.retain(|p| p.life > 0.0);
}

/// Stars rushing past, faster with the bass.
fn starfield(st: &mut SceneState, f: &mut Frame) {
    if st.stars.is_empty() {
        for _ in 0..260 {
            let star = [st.rng.centered() * 2.0, st.rng.centered() * 2.0, st.rng.next() * 0.9 + 0.1];
            st.stars.push(star);
        }
    }
    let speed = (0.004 + f.bass() * 0.035) * f.k;
    for i in 0..st.stars.len() {
        let mut star = st.stars[i];
        star[2] -= speed;
        if star[2] <= 0.02 {
            star = [st.rng.centered() * 2.0, st.rng.centered() * 2.0, 1.0];
        }
        st.stars[i] = star;
        let k2 = 0.5 / star[2];
        let at = f.c + Vec2::new(star[0] * k2 * f.w, star[1] * k2 * f.h);
        if !f.rect.contains(at) {
            continue;
        }
        let near = 1.0 - star[2];
        let color = if near > 0.6 { f.look.glow } else { f.look.cyan };
        f.pen.glow_dot(at, (near * 4.0 + 0.8) * f.s.max(0.5), color, (near + 0.2).min(1.0));
    }
}

/// Eight mirrored slices of spectrum rays, turning.
fn kaleido(st: &mut SceneState, f: &mut Frame) {
    st.time += 0.005 * f.k;
    const SEGMENTS: usize = 8;
    let r2 = f.w.min(f.h) * 0.5;
    let bins = f.audio.spectrum.len() as f32;
    for seg in 0..SEGMENTS {
        let turn = st.time + seg as f32 / SEGMENTS as f32 * TAU;
        let mirror = if seg % 2 == 1 { -1.0 } else { 1.0 };
        for i in 0..40 {
            let a = i as f32 / 40.0 * (TAU / SEGMENTS as f32);
            let m = f.fb((i as f32 * bins * 0.4 / 40.0) as usize).powf(1.3);
            if m < 0.01 {
                continue;
            }
            let dir = Vec2::angled(turn + mirror * a);
            let r0 = r2 * 0.12;
            let color = if i % 2 == 1 { f.look.glow } else { f.look.cyan };
            f.pen.glow_line(f.c + dir * r0, f.c + dir * (r0 + m * r2 * 0.55), 3.0 * f.s.max(0.4), color, 1.0);
        }
    }
}

/// The default: a glowing core, spectrum rays around it and the waveform as a ring.
fn radial(st: &mut SceneState, f: &mut Frame) {
    let bass = f.bass();
    let r = f.w.min(f.h) * 0.2;
    let core = r * (1.3 + bass * 1.8);
    let middle = glow(f.look.glow, 0.1 * 1.4);
    f.pen.radial(f.c, core * 0.4, glow(f.look.glow, 0.5 * 1.2), middle);
    f.pen.band(f.c, core * 0.4, core, middle, Color32::TRANSPARENT);

    const BARS: usize = 104;
    let half = f.audio.spectrum.len() as f32 * 0.5;
    let width = (f.w / 700.0).max(2.0 * f.s.max(0.4));
    for i in 0..BARS {
        let a = i as f32 / BARS as f32 * TAU - PI / 2.0;
        let m = f.fb((i as f32 * half / BARS as f32) as usize).powf(1.3);
        if m < 0.01 {
            continue;
        }
        let dir = Vec2::angled(a);
        let color = if i % 3 == 0 { f.look.cyan } else { f.look.glow };
        f.pen.glow_line(f.c + dir * r, f.c + dir * (r + m * r * 1.6), width, color, 1.0);
    }

    let wave = &f.audio.wave;
    st.points.clear();
    for i in (0..wave.len()).step_by(4) {
        let a = i as f32 / wave.len() as f32 * TAU;
        st.points.push(f.c + Vec2::angled(a) * (r * 0.66 + wave[i] * r * 0.2));
    }
    if let Some(first) = st.points.first().copied() {
        st.points.push(first);
    }
    f.pen.glow_polyline(&st.points, 2.0 * f.s.max(0.4), f.look.amber, 1.0);
}
