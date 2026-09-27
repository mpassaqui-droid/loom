//! What all scenes share (the end of the browser's `drawStage()`): each note
//! fires its voice's `| viz` style (burst, ring, beam, flash, shape), strong
//! bass hits send a shockwave ring, loud passages shed a little dust, and a
//! vignette darkens the edges.
//!
//! Positions and sizes are kept in "design pixels" (screen pixels divided by
//! the frame's scale), so a burst looks the same in the thumbnail and full screen.

use std::f32::consts::TAU;

use eframe::egui::{Color32, Pos2, Rect, Vec2};

use super::logic::{VoiceLook, Viz};
use super::pen::{alpha, glow, hsl, Pen};
use super::scenes::Frame;
use super::Look;

/// Small, fast random numbers (xorshift), good enough for sparkles.
pub struct Rng(u64);

impl Default for Rng {
    fn default() -> Self {
        Rng(0x2545_F491_4F6C_DD1D)
    }
}

impl Rng {
    /// A number in 0..1.
    pub fn next(&mut self) -> f32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 >> 40) as f32 / (1u64 << 24) as f32
    }

    /// A number in -0.5..0.5.
    pub fn centered(&mut self) -> f32 {
        self.next() - 0.5
    }
}

/// A moving dot that fades out.
#[derive(Clone, Copy)]
pub struct Particle {
    pub pos: Vec2,
    pub vel: Vec2,
    pub life: f32,
    /// None = the theme's glow/cyan colours.
    pub color: Option<Color32>,
}

struct Ring {
    radius: f32,
    alpha: f32,
    hue: Option<f32>,
}

struct Beam {
    angle: f32,
    life: f32,
    hue: f32,
}

struct Polygon {
    radius: f32,
    life: f32,
    hue: f32,
    sides: usize,
}

#[derive(Default)]
pub struct Effects {
    particles: Vec<Particle>,
    rings: Vec<Ring>,
    beams: Vec<Beam>,
    shapes: Vec<Polygon>,
    flash: Option<(f32, f32)>,
    last_bass: f32,
    rng: Rng,
}

impl Effects {
    /// Fire this frame's notes, then move and draw every effect (relative to the centre).
    pub fn draw(&mut self, f: &mut Frame, voices: &[VoiceLook]) {
        let (w, h) = (f.w / f.s, f.h / f.s); // design size
        let big = w.max(h);
        let r = w.min(h) * 0.2;
        let k = f.k;
        let levels = f.audio.levels;

        // bass shockwave
        if levels.bass > 0.55 && levels.bass - self.last_bass > 0.12 {
            self.rings.push(Ring { radius: r * 0.85, alpha: 0.55, hue: None });
        }
        self.last_bass = levels.bass;

        // each note, in its voice's style
        for hit in f.hits {
            let viz = voices.iter().find(|v| v.id == hit.voice).map_or(Viz::Burst, |v| v.viz);
            match viz {
                Viz::Ring => self.rings.push(Ring { radius: r * 0.4, alpha: 0.6, hue: Some(hit.hue) }),
                Viz::Beam => self.beams.push(Beam { angle: hit.hue / 360.0 * TAU, life: 1.0, hue: hit.hue }),
                Viz::Flash => self.flash = Some((hit.hue, 0.5 * (hit.gain + 0.3).min(1.0))),
                Viz::Shape => self.shapes.push(Polygon { radius: r * 0.3, life: 1.0, hue: hit.hue, sides: 3 + (hit.hue / 72.0) as usize }),
                Viz::Burst => self.burst(hit.hue, hit.gain, big),
            }
        }
        // a little dust when it's loud
        if levels.level > 0.12 && self.particles.len() < 500 && self.rng.next() < 0.5 {
            for _ in 0..2 {
                let a = self.rng.next() * TAU;
                let speed = big * 0.004 * (0.4 + self.rng.next());
                self.particles.push(Particle { pos: Vec2::ZERO, vel: Vec2::angled(a) * speed, life: 1.0, color: None });
            }
        }
        if self.particles.len() > 700 {
            self.particles.drain(..self.particles.len() - 700);
        }

        let c = f.c;
        let at = |v: Vec2| c + v * f.s;
        let s = f.s;
        let pen = &mut *f.pen;

        let grow = big * 0.013;
        self.rings.retain_mut(|ring| {
            ring.radius += grow * k;
            ring.alpha -= 0.012 * k;
            if ring.alpha <= 0.0 {
                return false;
            }
            let color = ring.hue.map_or(f.look.cyan, |hue| hsl(hue, 0.9, 0.6));
            pen.glow_ring(c, ring.radius * s, (1.5f32).max(f.w / 900.0), color, ring.alpha);
            true
        });

        let reach = (f.w * f.w + f.h * f.h).sqrt() / 2.0;
        self.beams.retain_mut(|beam| {
            beam.life -= 0.05 * k;
            if beam.life <= 0.0 {
                return false;
            }
            let end = c + Vec2::angled(beam.angle) * reach;
            pen.glow_line(c, end, (2.0 + beam.life * 5.0) * s, hsl(beam.hue, 0.9, 0.62), beam.life * 0.7);
            true
        });

        let shape_grow = big * 0.006;
        self.shapes.retain_mut(|shape| {
            shape.radius += shape_grow * k;
            shape.life -= 0.03 * k;
            if shape.life <= 0.0 {
                return false;
            }
            let color = hsl(shape.hue, 0.9, 0.62);
            let turn = shape.radius * 0.002;
            let corner = |i: usize| c + Vec2::angled(i as f32 / shape.sides as f32 * TAU + turn) * shape.radius * s;
            for i in 0..shape.sides {
                pen.glow_line(corner(i), corner(i + 1), 2.5 * s, color, shape.life);
            }
            true
        });

        let damping = 0.97f32.powf(k);
        let (glow_color, cyan) = (f.look.glow, f.look.cyan);
        for (i, p) in self.particles.iter_mut().enumerate() {
            p.pos += p.vel * k;
            p.vel *= damping;
            p.life -= 0.016 * k;
            if p.life <= 0.0 {
                continue;
            }
            let (color, strength) = match p.color {
                Some(color) => (color, p.life),
                None => (if i % 2 == 1 { glow_color } else { cyan }, p.life * 0.9),
            };
            pen.glow_dot(at(p.pos), (2.0 + p.life * 2.8) * s, color, strength);
        }
        self.particles.retain(|p| p.life > 0.0);

        if let Some((hue, life)) = &mut self.flash {
            *life -= 0.04 * k;
            if *life > 0.0 {
                pen.rect(f.rect, glow(hsl(*hue, 0.9, 0.55), *life * 0.22 * 1.6));
            }
        }
        if self.flash.is_some_and(|(_, life)| life <= 0.0) {
            self.flash = None;
        }
    }

    /// The browser's `emitVoiceBurst`: particles shot in the voice's own direction.
    fn burst(&mut self, hue: f32, gain: f32, big: f32) {
        let color = hsl(hue, 0.9, 0.62);
        let count = 4 + (gain * 10.0) as usize;
        let base = hue / 360.0 * TAU;
        let speed = big * (0.006 + gain * 0.012);
        for _ in 0..count {
            let a = base + self.rng.centered() * 1.1;
            let v = speed * (0.5 + self.rng.next());
            self.particles.push(Particle { pos: Vec2::ZERO, vel: Vec2::angled(a) * v, life: 1.0, color: Some(color) });
        }
    }
}

/// Darker edges, like a camera lens (drawn over the scene, under the text).
/// `pump` (0..1) opens the lens when the music is loud, so the frame breathes.
pub fn vignette(pen: &mut Pen, rect: Rect, pump: f32) {
    let (w, h) = (rect.width(), rect.height());
    let c: Pos2 = rect.center();
    let inner = w.min(h) * (0.28 + 0.10 * pump.clamp(0.0, 1.0));
    pen.band(c, inner, w.max(h) * 0.72, Color32::TRANSPARENT, alpha(Color32::BLACK, 0.55));
}

/// Post-FX shared by every scene: the whole picture moves with the music —
/// the camera kicks and the screen flashes on a bass hit, and a halo swells
/// with the level. All of it decays to nothing, so a silent stage looks exactly
/// as it did before (and the thumbnail stays calm too).
#[derive(Default)]
pub struct PostFx {
    /// Camera shake 0..1.
    shake: f32,
    /// Screen flash 0..1.
    strobe: f32,
    /// Smoothed loudness driving the halo and the vignette (0..1).
    pump: f32,
    last_bass: f32,
    rng: Rng,
}

impl PostFx {
    /// Read the music once per frame, before anything is drawn: a bass hit
    /// (the same rise the shockwave ring watches) kicks the camera and flashes
    /// the screen; the overall level feeds the halo.
    pub fn update(&mut self, k: f32, bass: f32, level: f32) {
        if bass > 0.5 && bass - self.last_bass > 0.1 {
            let hit = ((bass - 0.5) * 2.0).clamp(0.0, 1.0);
            self.shake = self.shake.max(0.35 + hit * 0.65);
            self.strobe = self.strobe.max(0.20 + hit * 0.45);
        }
        self.last_bass = bass;
        self.shake *= 0.82f32.powf(k);
        self.strobe *= 0.74f32.powf(k);
        // the halo rises fast and falls slowly, like an optical compressor
        let target = level.clamp(0.0, 1.0);
        let rate: f32 = if target > self.pump { 0.35 } else { 0.08 };
        self.pump += (target - self.pump) * (1.0 - (1.0 - rate).powf(k));
    }

    /// `rect` displaced by this frame's shake (at most ~1 % of the stage, so the
    /// background still covers the edges). Unchanged while the shake is spent.
    pub fn shaken(&mut self, rect: Rect) -> Rect {
        if self.shake <= 0.001 {
            return rect;
        }
        let reach = rect.width().min(rect.height()) * 0.012 * self.shake;
        let jitter = Vec2::new(self.rng.centered() * 2.0 * reach, self.rng.centered() * 2.0 * reach);
        rect.translate(jitter)
    }

    /// How much the lens breathes this frame (0..1).
    pub fn pump(&self) -> f32 {
        self.pump
    }

    /// The halo that swells with the level, under the vignette.
    pub fn draw_bloom(&self, pen: &mut Pen, rect: Rect, look: &Look) {
        if self.pump <= 0.01 {
            return;
        }
        let radius = rect.width().min(rect.height()) * (0.30 + 0.35 * self.pump);
        pen.radial(rect.center(), radius, glow(look.glow, self.pump * 0.16), Color32::TRANSPARENT);
    }

    /// The flash on a bass hit, over everything else.
    pub fn draw_strobe(&self, pen: &mut Pen, rect: Rect, look: &Look) {
        if self.strobe > 0.002 {
            pen.rect(rect, glow(look.glow, self.strobe * 0.28));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A bass hit must kick the camera and the flash, and silence must bring
    /// everything back to rest (a quiet stage looks like it always did).
    #[test]
    fn post_fx_kicks_on_bass_then_settles() {
        let mut fx = PostFx::default();
        fx.update(1.0, 0.0, 0.0);
        assert_eq!((fx.shake, fx.strobe), (0.0, 0.0), "silence must not move the picture");

        fx.update(1.0, 0.9, 0.5); // a rise from 0.0 to 0.9 = a hit
        assert!(fx.shake > 0.5 && fx.strobe > 0.3, "a bass hit must kick ({}, {})", fx.shake, fx.strobe);

        // holding the same loud bass is NOT a new hit, so it decays
        let kicked = fx.shake;
        for _ in 0..60 {
            fx.update(1.0, 0.9, 0.0);
        }
        assert!(fx.shake < kicked * 0.01, "the shake must die away (got {})", fx.shake);
        assert!(fx.strobe < 0.002, "the flash must die away (got {})", fx.strobe);
    }

    /// The halo rises with the level and falls back slower.
    #[test]
    fn pump_follows_the_level() {
        let mut fx = PostFx::default();
        for _ in 0..40 {
            fx.update(1.0, 0.0, 1.0);
        }
        assert!(fx.pump() > 0.9, "loud music must open the lens (got {})", fx.pump());
        let loud = fx.pump();
        for _ in 0..5 {
            fx.update(1.0, 0.0, 0.0);
        }
        assert!(fx.pump() < loud, "it must come back down");
        assert!(fx.pump() > 0.2, "…but slowly (got {})", fx.pump());
    }

    /// A spent shake leaves the frame exactly where it was (no jitter at rest).
    #[test]
    fn shake_moves_the_frame_only_while_alive() {
        let rect = Rect::from_min_size(Pos2::ZERO, Vec2::new(1000.0, 800.0));
        let mut fx = PostFx::default();
        assert_eq!(fx.shaken(rect), rect, "at rest the frame must not move");

        fx.update(1.0, 0.9, 0.0);
        let moved = fx.shaken(rect);
        assert_ne!(moved, rect, "a live shake must displace the frame");
        // and never far enough to show a seam: at most ~1.2 % of the short side
        let reach = 800.0 * 0.012 + 0.001;
        assert!(moved.min.x.abs() <= reach && moved.min.y.abs() <= reach, "shake too wide: {moved:?}");
    }
}
