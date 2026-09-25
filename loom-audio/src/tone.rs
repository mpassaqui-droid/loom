//! Synthesized notes — the port of the browser's `tone()` SOURCE half, up to
//! (not including) the insert chain. Same numbers, same signal order:
//!
//! ```text
//! unison oscs / pluck / bow / reed / sync / pwm / bell / grain ── ×1/√unison ─┐
//! sub, double, harmonize ─────────────────────────────────────────────────────┤
//!                                                        [filter + fenv + LFO] ┴─┐
//! noise bed (high-passed) ───────────────────────────────────────────────────────┴─ × ADSR ─► out
//! ```
//!
//! Pitch motion (`slide`, `bend`), `fm`, `wow` and pitch LFOs move the oscillators.
//!
//! For speed, a note is synthesized [`BLOCK`] samples at a time: once per block
//! the envelopes, pitch motion and filter are updated ("control rate", linearly
//! interpolated across the block), then tight loops render the block, and
//! `render` hands the samples out one by one.

mod sources;
mod table;

use crate::clock::{name_salt, Clock};
use crate::dsp::{exp_ramp, lin_ramp, osc, Biquad, BiquadKind, Noise};
use crate::note::{NoteSpec, Stereo};
use loom_core::{Env, FilterType, Param, Wave};
use sources::{advance, equal_power, Bell, BufferPlayer, Grains, Pwm};
use std::f32::consts::TAU;
use table::{SineTable, WaveTable};

/// Samples per block (and between control updates).
const BLOCK: usize = 16;

/// What makes the main sound of the note.
enum Source {
    /// The unison oscillators (basic wave, `| wt` or `| partials` table).
    Osc { voices: Vec<Voice>, wave: Wave, table: Option<WaveTable> },
    /// `pluck`, `bow`, `reed`, `sync`.
    Buffer(BufferPlayer),
    Pwm(Pwm),
    Bell(Bell),
    Grains(Grains),
}

/// One unison oscillator.
struct Voice {
    phase: f32,
    /// Static detune as a frequency multiplier.
    det: f32,
    /// Stereo gains (`| spread`), 1/1 when mono.
    gl: f32,
    gr: f32,
}

/// A mono layer: `sub` (sine an octave down), `double` (octave up), `harmonize`.
struct Layer { phase: f32, ratio: f32, level: f32, wave: Wave, sub: bool }

/// `| fm RATIO INDEX FB`: a sine operator (optionally modulated by a second one)
/// whose output, in Hz, is added to the carriers' frequency.
struct FmOp {
    phase: f32,
    dt: f32,
    /// Index envelope: `depth` Hz decaying to 35% over `decay` seconds.
    depth: f32,
    decay: f32,
    /// Second operator: (phase, dt, depth in cycles/sample).
    op2: Option<(f32, f32, f32)>,
}

/// `| noise N`: white noise → high-pass 1100 Hz → level. The browser's noise
/// buffer is 0.4 s long and not looped, so the bed stops after 0.4 s.
struct NoiseBed { noise: Noise, hp: Biquad, level: f32, end: u32 }

/// The `| lp/hp/bp/notch` filter with its envelope.
struct Filter {
    kind: BiquadKind,
    /// WebAudio reads Q in dB for lowpass/highpass, as a plain ratio otherwise.
    q_in_db: bool,
    q: f32,
    fenv: Option<f32>,
    /// Filter envelope attack and decay (the note's `| env`, or 5 ms / 180 ms).
    fe_a: f32,
    fe_d: f32,
    l: Biquad,
    r: Biquad,
    /// Last (frequency, Q) given to the biquads, to skip needless recomputes.
    last: (f32, f32),
}

/// An `| lfo` aimed at something the source owns. Shapes follow WebAudio's
/// oscillator types (`rand` plays a square, like the browser).
struct ToneLfo { shape: LfoWave, rate: f32, target: LfoTarget }

#[derive(Clone, Copy)]
enum LfoWave { Sine, Tri, Saw, Square }

#[derive(Clone, Copy)]
enum LfoTarget {
    /// Swing as a fraction of the cutoff (JS `cutoff · min(0.95, depth)`).
    Cutoff(f32),
    /// Swing added to Q (JS `min(20, depth·20)`).
    Q(f32),
    /// Swing in cents (JS `clamp(depth·100, 0, 400)`).
    Pitch(f32),
}

/// The values that move smoothly across a block.
#[derive(Clone, Copy, Default)]
struct Ctrl {
    amp: f32,
    /// Slide/bend frequency ratio for the carriers, and for `sub`.
    glide: f32,
    sub_glide: f32,
    /// Detune multiplier from `wow` and pitch LFOs.
    pitch: f32,
    /// FM index envelope, in Hz.
    fm: f32,
}

impl Ctrl {
    /// Per-sample steps to go from `self` to `to` in one block.
    fn step_to(&self, to: &Ctrl) -> Ctrl {
        let k = 1.0 / BLOCK as f32;
        Ctrl {
            amp: (to.amp - self.amp) * k,
            glide: (to.glide - self.glide) * k,
            sub_glide: (to.sub_glide - self.sub_glide) * k,
            pitch: (to.pitch - self.pitch) * k,
            fm: (to.fm - self.fm) * k,
        }
    }
}

/// A frequency ramp: ratio `from` → 1 over `time` seconds (WebAudio exponential ramp).
#[derive(Clone, Copy)]
struct Glide { from: f32, time: f32 }

impl Glide {
    fn at(g: Option<Glide>, t: f32) -> f32 {
        g.map_or(1.0, |g| exp_ramp(g.from, 1.0, 0.0, g.time, t))
    }
}

pub struct Tone {
    sr: f32,
    /// Samples handed out so far, and the sample where the JS stops the note.
    n: u32,
    stop: u32,
    peak: f32,
    dur: f32,
    env: Option<Env>,

    source: Source,
    /// JS `uniGain`: 1/√unison on the main source.
    uni_gain: f32,
    stereo: bool,
    layers: Vec<Layer>,
    noise: Option<NoiseBed>,
    fm: Option<FmOp>,

    /// Note frequency / sample rate.
    dt0: f32,
    glide: Option<Glide>,
    sub_glide: Option<Glide>,
    /// `wow`: (rate Hz, depth cents).
    wow: Option<(f32, f32)>,
    lfos: Vec<ToneLfo>,

    cutoff: Option<Param>,
    over: Option<f32>,
    salt: u32,
    filter: Option<Filter>,

    sine: &'static SineTable,
    /// Samples synthesized so far (always a whole number of blocks).
    made: u32,
    /// Control values at the start of the next block.
    next: Ctrl,
    /// The current block and the read position in it.
    out_l: [f32; BLOCK],
    out_r: [f32; BLOCK],
    pos: usize,
}

impl Tone {
    pub fn new(spec: &NoteSpec) -> Tone {
        let s = spec.synth;
        let sr = spec.sr;
        let freq = if spec.freq.is_finite() && spec.freq > 0.0 { spec.freq } else { 1.0 };
        let dur = spec.dur.max(0.0);
        let mut noise = Noise::new(spec.seed);

        // --- which source (JS: the first match wins; a 0 counts as "off") ---
        let pluck = s.pluck.filter(|b| *b != 0.0);
        let sync = s.sync.filter(|r| *r != 0.0);
        let pwm = s.pwm.filter(|p| *p != 0.0);
        let grain = s.grain.filter(|g| *g != 0.0);
        let exclusive = pluck.is_some() || s.bow || s.reed || sync.is_some() || pwm.is_some() || s.bell || grain.is_some();

        let uni = s.unison.unwrap_or(1).clamp(1, 7);
        let det = s.detune.unwrap_or(if uni > 1 { 14.0 } else { 0.0 });
        let spread = s.spread.filter(|x| *x != 0.0 && uni > 1);

        // --- pitch motion: slide from the previous note, else bend ---
        let slide_t = s.slide.map_or(0.0, |x| x.clamp(0.005, 0.5));
        let from_f = spec.prev_freq.filter(|f| slide_t > 0.0 && *f > 0.0);
        let bend_start = s.bend.map(|b| (freq * 2f32.powf(b.semi / 12.0)).max(20.0));
        let start_f = from_f.or(bend_start);
        let ramp_t = match (from_f, s.bend) {
            (Some(_), _) => slide_t.min(dur * 0.95),
            (None, Some(b)) => b.time.min(dur * 0.9),
            _ => 0.0,
        };
        let glide = start_f.filter(|_| ramp_t > 0.0).map(|f| Glide { from: f / freq, time: ramp_t });
        // the sub follows the slide only (not the bend)
        let sub_glide = from_f.map(|f| Glide { from: f / freq, time: slide_t.min(dur * 0.95) }).filter(|g| g.time > 0.0);

        let source = if let Some(bright) = pluck {
            Source::Buffer(BufferPlayer::pluck(sr, freq, dur, bright, spec.seed))
        } else if s.bow {
            Source::Buffer(BufferPlayer::bow(sr, freq, dur, spec.seed))
        } else if s.reed {
            Source::Buffer(BufferPlayer::reed(sr, freq, dur, spec.seed))
        } else if let Some(ratio) = sync {
            Source::Buffer(BufferPlayer::sync(sr, freq, dur, ratio))
        } else if let Some(amount) = pwm {
            Source::Pwm(Pwm::new(sr, freq, amount))
        } else if s.bell {
            Source::Bell(Bell::new(sr, freq, dur))
        } else if let Some(amount) = grain {
            Source::Grains(Grains::new(sr, freq, dur, amount, spec.wave, &mut noise))
        } else {
            let voices = (0..uni)
                .map(|i| {
                    let cents = if uni > 1 { det * (i as f32 - (uni - 1) as f32 / 2.0) } else { 0.0 };
                    let (gl, gr) = match spread {
                        Some(sp) => equal_power((i as f32 / (uni - 1) as f32 * 2.0 - 1.0) * sp),
                        None => (1.0, 1.0),
                    };
                    Voice { phase: 0.0, det: 2f32.powf(cents / 1200.0), gl, gr }
                })
                .collect();
            let harmonics = match (&s.partials, &s.wt) {
                (Some(p), _) if !p.is_empty() => Some(p.clone()),
                (_, Some(name)) => Some(table::named_harmonics(name)),
                _ => None,
            };
            // keep only the harmonics that stay below Nyquist at this note's highest pitch
            let top_f = freq.max(start_f.unwrap_or(0.0)) * 2f32.powf((det * (uni - 1) as f32 / 2.0 + 22.0) / 1200.0);
            let max_harm = ((sr * 0.5 / top_f) as usize).max(1);
            let table = harmonics.map(|h| WaveTable::new(&h, max_harm));
            Source::Osc { voices, wave: spec.wave, table }
        };
        let stereo = matches!(source, Source::Grains(_)) || (matches!(source, Source::Osc { .. }) && spread.is_some());

        // --- layers: sub, double, harmonize (not with the special sources) ---
        let mut layers = Vec::new();
        if !exclusive {
            if let Some(level) = s.sub.filter(|x| *x > 0.0) {
                layers.push(Layer { phase: 0.0, ratio: 0.5, level: level.min(1.0), wave: Wave::Sine, sub: true });
            }
            if let Some(level) = s.double.filter(|x| *x > 0.0) {
                layers.push(Layer { phase: 0.0, ratio: 2.0, level: level.min(1.0) * 0.5, wave: spec.wave, sub: false });
            }
            if let Some(h) = s.harm2.filter(|h| h.lvl > 0.0) {
                layers.push(Layer { phase: 0.0, ratio: 2f32.powf(h.semi as f32 / 12.0), level: h.lvl * 0.5, wave: spec.wave, sub: false });
            }
        }

        let fm = s.fm.filter(|f| f.index > 0.0 && !exclusive).map(|f| FmOp {
            phase: 0.0,
            dt: freq * f.ratio / sr,
            depth: (freq * f.ratio * f.index).max(0.001),
            decay: s.env.map_or(0.18, |e| e.d.max(0.02)),
            op2: (f.fb > 0.0).then(|| (0.0, freq * f.ratio * f.fb / sr, freq * f.ratio * f.fb * f.index / sr)),
        });

        let noise_bed = s.noise.filter(|x| *x > 0.0).map(|x| NoiseBed {
            noise: Noise::new(spec.seed ^ 0x9e37_79b9),
            hp: Biquad::highpass(sr, 1100.0, db_q(1.0)),
            level: x.min(1.0) * 0.5,
            end: (0.4 * sr) as u32,
        });

        let lfos = [&s.lfo, &s.lfo2].into_iter().flatten().filter_map(|cfg| {
            let depth = cfg.depth;
            let target = match cfg.target.as_str() {
                "cutoff" => LfoTarget::Cutoff(depth.min(0.95)),
                "q" | "reso" | "res" => LfoTarget::Q((depth * 20.0).min(20.0)),
                "pitch" => LfoTarget::Pitch((depth * 100.0).clamp(0.0, 400.0)),
                _ => return None, // gain/vol/pan live in the insert chain
            };
            let shape = match cfg.shape.as_str() {
                "tri" | "triangle" => LfoWave::Tri,
                "saw" | "sawtooth" => LfoWave::Saw,
                "square" | "sqr" | "rand" | "random" => LfoWave::Square,
                _ => LfoWave::Sine,
            };
            let rate = if cfg.rate != 0.0 { cfg.rate } else { 4.0 };
            Some(ToneLfo { shape, rate: rate.max(0.01), target })
        }).collect();

        let filter = s.cutoff.map(|_| {
            let kind = match s.filter_type.unwrap_or(FilterType::Lowpass) {
                FilterType::Lowpass => BiquadKind::Lowpass,
                FilterType::Highpass => BiquadKind::Highpass,
                FilterType::Bandpass => BiquadKind::Bandpass,
                FilterType::Notch => BiquadKind::Notch,
            };
            let fe = s.env.unwrap_or(Env { a: 0.005, d: 0.18, s: 0.4, r: 0.2 });
            Filter {
                kind,
                q_in_db: matches!(kind, BiquadKind::Lowpass | BiquadKind::Highpass),
                q: s.q.unwrap_or(1.0),
                fenv: s.fenv.filter(|x| *x > 0.0),
                fe_a: fe.a,
                fe_d: fe.d,
                l: Biquad::lowpass(sr, 1000.0, 1.0),
                r: Biquad::lowpass(sr, 1000.0, 1.0),
                last: (f32::NAN, f32::NAN),
            }
        });

        // JS stop time: with `| env`, after the gate + release; without, after the pluck decay
        let stop = match s.env {
            Some(e) => (e.a.max(0.001) + e.d.max(0.001) + 0.001).max(dur) + e.r.max(0.01) + 0.02,
            None => (dur * 0.9).min(0.4) + 0.02,
        };

        let mut tone = Tone {
            sr,
            n: 0,
            stop: (stop * sr).ceil() as u32,
            peak: spec.peak,
            dur,
            env: s.env,
            source,
            uni_gain: 1.0 / (uni as f32).sqrt(),
            stereo,
            layers,
            noise: noise_bed,
            fm,
            dt0: freq / sr,
            glide,
            sub_glide,
            wow: s.wow.filter(|w| *w > 0.0).map(|w| (3.0 + w * 5.0, w * 22.0)),
            lfos,
            cutoff: s.cutoff,
            over: s.over,
            salt: name_salt(spec.name),
            filter,
            sine: table::sine(),
            made: 0,
            next: Ctrl::default(),
            out_l: [0.0; BLOCK],
            out_r: [0.0; BLOCK],
            pos: BLOCK,
        };
        tone.next = tone.ctrl_at(0.0);
        tone
    }

    pub fn done(&self) -> bool { self.n >= self.stop }

    pub fn render(&mut self, clock: &Clock) -> Stereo {
        if self.pos == BLOCK {
            self.render_block(clock);
            self.pos = 0;
        }
        let i = self.pos;
        self.pos += 1;
        self.n += 1;
        Stereo { l: self.out_l[i], r: self.out_r[i] }
    }

    /// Amplitude envelope at `t` seconds, exactly the browser's automation curves.
    fn amp_at(&self, t: f32) -> f32 {
        let peak = self.peak;
        match self.env {
            Some(e) => {
                let a_t = e.a.max(0.001);
                let d_t = a_t + e.d.max(0.001);
                let sus = (peak * e.s).max(0.0001);
                let off = (d_t + 0.001).max(self.dur);
                let rel = e.r.max(0.01);
                if t < a_t { lin_ramp(0.0001, peak, 0.0, a_t, t) }
                else if t < d_t { exp_ramp(peak, sus, a_t, d_t, t) }
                else if t < off { sus }
                else { exp_ramp(sus, 0.0001, off, off + rel, t) }
            }
            None => {
                let d = (self.dur * 0.9).min(0.4);
                if d >= 0.008 {
                    if t < 0.008 { exp_ramp(0.0001, peak, 0.0, 0.008, t) } else { exp_ramp(peak, 0.001, 0.008, d, t) }
                } else if t < d {
                    // very short notes: WebAudio sorts the ramps by time, so the decay target comes first
                    exp_ramp(0.0001, 0.001, 0.0, d, t)
                } else {
                    exp_ramp(0.001, peak, d, 0.008, t)
                }
            }
        }
    }

    /// Sum of the LFOs aimed at `pick`'s target at `t`, each scaled by its amount.
    fn lfo_sum(&self, t: f32, pick: fn(LfoTarget) -> Option<f32>) -> f32 {
        self.lfos.iter().filter_map(|l| pick(l.target).map(|amt| lfo_wave(l.shape, l.rate * t) * amt)).sum()
    }

    /// The smooth values at `t` seconds.
    fn ctrl_at(&self, t: f32) -> Ctrl {
        let mut cents = self.lfo_sum(t, |g| if let LfoTarget::Pitch(c) = g { Some(c) } else { None });
        if let Some((rate, depth)) = self.wow { cents += depth * (TAU * rate * t).sin(); }
        Ctrl {
            amp: self.amp_at(t),
            glide: Glide::at(self.glide, t),
            sub_glide: Glide::at(self.sub_glide, t),
            pitch: if cents != 0.0 { 2f32.powf(cents / 1200.0) } else { 1.0 },
            fm: self.fm.as_ref().map_or(0.0, |f| exp_ramp(f.depth, (f.depth * 0.35).max(0.001), 0.0, f.decay, t)),
        }
    }

    /// Point the filter at time `t` (cutoff param, filter envelope, LFOs).
    fn update_filter(&mut self, clock: &Clock, t: f32) {
        let (Some(cut), Some(f)) = (self.cutoff, &self.filter) else { return };
        let raw = clock.param(&cut, self.over, self.salt);
        // JS: no filter at all when the cutoff is 0 as the note starts
        if self.made == 0 && raw == 0.0 { self.filter = None; return; }
        let fc = raw.clamp(20.0, 20000.0);
        let mut freq = match f.fenv {
            Some(amt) => {
                let top = (fc * (1.0 + amt * 7.0)).min(18000.0);
                let bottom = (fc * (1.0 - amt * 0.85)).max(60.0);
                let a = f.fe_a.max(0.002);
                if t < a { lin_ramp(bottom, top, 0.0, a, t) } else { exp_ramp(top, fc.max(60.0), a, f.fe_a + f.fe_d.max(0.01), t) }
            }
            None => fc,
        };
        let mut q = f.q;
        if !self.lfos.is_empty() {
            freq += raw * self.lfo_sum(t, |g| if let LfoTarget::Cutoff(k) = g { Some(k) } else { None });
            q += self.lfo_sum(t, |g| if let LfoTarget::Q(k) = g { Some(k) } else { None });
        }
        let (sr, stereo) = (self.sr, self.stereo);
        let Some(f) = &mut self.filter else { return };
        if (freq, q) != f.last {
            f.last = (freq, q);
            let ql = if f.q_in_db { db_q(q) } else { q };
            f.l.set(f.kind, sr, freq.max(0.0), ql, 0.0);
            if stereo { f.r.set(f.kind, sr, freq.max(0.0), ql, 0.0); }
        }
    }

    /// Synthesize the next block into `out_l` / `out_r`.
    fn render_block(&mut self, clock: &Clock) {
        let t = self.made as f32 / self.sr;
        let c = self.next;
        self.next = self.ctrl_at(t + BLOCK as f32 / self.sr);
        let d = c.step_to(&self.next);
        self.update_filter(clock, t);

        let (sine, dt0, n0, stereo) = (self.sine, self.dt0, self.made, self.stereo);
        // i-th sample of a smoothly moving value
        let at = |v: f32, dv: f32, i: usize| v + dv * i as f32;
        let mut l = [0.0f32; BLOCK];
        let mut r = [0.0f32; BLOCK];

        // FM operator output, as a frequency offset in cycles per sample
        let mut fm = [0.0f32; BLOCK];
        if let Some(op) = &mut self.fm {
            let k = 1.0 / self.sr;
            for (i, y) in fm.iter_mut().enumerate() {
                let mut dt = op.dt;
                if let Some((ph2, dt2, amt2)) = &mut op.op2 {
                    dt += sine_osc(sine, *ph2, *dt2) * *amt2;
                    advance(ph2, *dt2);
                }
                *y = sine_osc(sine, op.phase, dt) * at(c.fm, d.fm, i) * k;
                advance(&mut op.phase, dt);
            }
        }

        // pitch (cycles per sample) of an undetuned carrier: glide, FM, wow/LFO
        let mut carrier = [0.0f32; BLOCK];
        for (i, dt) in carrier.iter_mut().enumerate() {
            *dt = (dt0 * at(c.glide, d.glide, i) + fm[i]) * at(c.pitch, d.pitch, i);
        }

        // --- main source (mono sources write the left channel only) ---
        match &mut self.source {
            Source::Osc { voices, wave, table } => {
                for v in voices.iter_mut() {
                    let dts = carrier.map(|dt| dt * v.det);
                    let mut x = [0.0f32; BLOCK];
                    match (table.as_ref(), *wave) {
                        (Some(tb), _) => run_osc(&mut v.phase, &dts, &mut x, |p, _| tb.read(p)),
                        (None, Wave::Sine) => run_osc(&mut v.phase, &dts, &mut x, |p, _| sine.at(p)),
                        (None, Wave::Saw) => run_osc(&mut v.phase, &dts, &mut x, |p, dt| osc(Wave::Saw, p, dt)),
                        (None, w) => run_osc(&mut v.phase, &dts, &mut x, |p, dt| osc(w, p, dt)),
                    }
                    if stereo {
                        for i in 0..BLOCK { l[i] += x[i] * v.gl; r[i] += x[i] * v.gr; }
                    } else {
                        for i in 0..BLOCK { l[i] += x[i]; }
                    }
                }
            }
            Source::Buffer(b) => (0..BLOCK).for_each(|i| l[i] = b.next(at(c.pitch, d.pitch, i))),
            Source::Pwm(p) => (0..BLOCK).for_each(|i| l[i] = p.next(at(c.pitch, d.pitch, i), sine)),
            Source::Bell(b) => {
                let pitch: [f32; BLOCK] = std::array::from_fn(|i| at(c.pitch, d.pitch, i));
                b.render(&pitch, &mut l, sine);
            }
            Source::Grains(g) => (0..BLOCK).for_each(|i| (l[i], r[i]) = g.next(n0 + i as u32)),
        }
        let ug = self.uni_gain;
        l.iter_mut().for_each(|x| *x *= ug);
        if stereo { r.iter_mut().for_each(|x| *x *= ug); }

        // --- layers (mono, into the filter too) ---
        for ly in &mut self.layers {
            let (g, dg) = if ly.sub { (c.sub_glide, d.sub_glide) } else { (c.glide, d.glide) };
            for i in 0..BLOCK {
                let dt = (dt0 * ly.ratio * at(g, dg, i) + fm[i]) * at(c.pitch, d.pitch, i);
                let x = if dt.abs() < 0.5 { wave_at(sine, ly.wave, ly.phase, dt) * ly.level } else { 0.0 };
                advance(&mut ly.phase, dt);
                l[i] += x;
                if stereo { r[i] += x; }
            }
        }

        // --- filter ---
        if let Some(f) = &mut self.filter {
            l.iter_mut().for_each(|x| *x = f.l.process(*x));
            if stereo { r.iter_mut().for_each(|x| *x = f.r.process(*x)); }
        }
        if !stereo { r = l; }

        // --- noise bed (after the filter), then the amplitude envelope ---
        if let Some(nb) = &mut self.noise {
            if n0 < nb.end {
                for i in 0..BLOCK {
                    let x = nb.hp.process(nb.noise.sample()) * nb.level;
                    l[i] += x;
                    r[i] += x;
                }
            }
        }
        for i in 0..BLOCK {
            let a = at(c.amp, d.amp, i);
            self.out_l[i] = l[i] * a;
            self.out_r[i] = r[i] * a;
        }
        self.made += BLOCK as u32;
    }
}

/// WebAudio's lowpass/highpass Q is a resonance in dB; the biquad wants a ratio.
fn db_q(q_db: f32) -> f32 { 10f32.powf(q_db / 20.0) }

/// Run one oscillator for a block: `dts[i]` = cycles per sample, `wave(phase, dt)`
/// its shape. Generic so each shape gets its own branch-free loop.
#[inline(always)]
fn run_osc(phase: &mut f32, dts: &[f32; BLOCK], out: &mut [f32; BLOCK], wave: impl Fn(f32, f32) -> f32) {
    let mut ph = *phase;
    for (o, &dt) in out.iter_mut().zip(dts) {
        // above Nyquist: silent, like WebAudio's band-limited oscillators
        *o = if dt.abs() < 0.5 { wave(ph, dt) } else { 0.0 };
        advance(&mut ph, dt);
    }
    *phase = ph;
}

/// A basic waveform sample, using the fast sine table for sines.
#[inline]
fn wave_at(sine: &SineTable, wave: Wave, phase: f32, dt: f32) -> f32 {
    if wave == Wave::Sine { sine.at(phase) } else { osc(wave, phase, dt) }
}

/// A sine oscillator sample (silent above Nyquist, like WebAudio's band-limited tables).
#[inline]
fn sine_osc(sine: &SineTable, phase: f32, dt: f32) -> f32 {
    if dt.abs() >= 0.5 { 0.0 } else { sine.at(phase) }
}

/// WebAudio oscillator shapes for LFOs, -1..1, at `cycles` cycles since the note started.
/// Their phases match WebAudio: every shape starts at 0 and rises, except square (+1).
fn lfo_wave(shape: LfoWave, cycles: f32) -> f32 {
    let p = cycles - cycles.floor();
    match shape {
        LfoWave::Sine => (TAU * p).sin(),
        LfoWave::Square => if p < 0.5 { 1.0 } else { -1.0 },
        LfoWave::Saw => { let q = p + 0.5; 2.0 * (q - q.floor()) - 1.0 }
        LfoWave::Tri => { let q = p + 0.25; 1.0 - 4.0 * ((q - q.floor()) - 0.5).abs() }
    }
}

#[cfg(test)]
mod tests;
