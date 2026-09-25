//! The mix: buses, master reverb and delay, sidechain duck, and the mastering
//! chain — the port of the browser's `buildGraph()` and `busFor()`.
//!
//! ```text
//! voices ─► bus: submixes (gain → lp/hp) ─┐        ┌─ reverb send ─► SPACE reverb ─ ×0.18 ─┐
//!           main bus ◄────────────────────┴────────┼─ full ────────► ECHO ping-pong ─ ×0.15 ─┤
//! voice `| reverb` / `| delay` sends ───────────────┘                                        │
//!           main bus ─► duck (scGain) ───────────────────────────────────────────────────────┤
//!                                                                                            ▼
//!   volume → HPF 28 Hz → glue comp → EQ (120 / 1k / 8k) → warmth sat → limiter comp → lookahead limiter → out
//! ```
//!
//! [`dynamics`]: compressors, saturator, limiter. [`reverb`]: Schroeder / FDN and the ping-pong delay.

pub(crate) mod dynamics;
pub(crate) mod reverb;

use crate::dsp::{Biquad, BiquadKind};
use crate::inserts::prims::q_from_db;
use crate::note::Stereo;
use dynamics::{Compressor, Limiter, Saturator};
use loom_core::{BusConfig, MixSettings, Room};
use reverb::{PingPong, Reverb};

/// The SPACE slider's default (20) → `space/100*0.9`.
const SPACE_SEND: f32 = 0.20 * 0.9;
/// The ECHO slider's default (15) → `echo/100`.
const ECHO_SEND: f32 = 0.15;
/// The master EQ sliders' defaults (dB). The browser's high shelf reads the
/// `eqHigh` slider (default 2); its 2.5 dB fallback only applies without the slider.
const EQ_LOW_DB: f32 = 0.0;
const EQ_MID_DB: f32 = 0.0;
const EQ_HIGH_DB: f32 = 2.0;

/// Everything the notes sent to the master during one sample.
#[derive(Clone, Debug, Default)]
pub struct MixInput {
    /// One stereo sum per bus (index 0 = main).
    pub buses: Vec<Stereo>,
    /// Sum of the `| reverb` sends.
    pub reverb: Stereo,
    /// Sum of the `| delay` sends.
    pub delay: Stereo,
}

impl MixInput {
    pub fn clear(&mut self) {
        self.buses.iter_mut().for_each(|b| *b = Stereo::ZERO);
        self.reverb = Stereo::ZERO;
        self.delay = Stereo::ZERO;
    }
}

/// A `bus: NAME gain G lp F hp F q Q reverb R` submix.
struct Bus {
    cfg: BusConfig,
    gain: f32,
    /// One filter: highpass if `hp` is given, else lowpass if `lp` is.
    filter: Option<[Biquad; 2]>,
    /// Send from the filtered bus to the reverb.
    reverb: f32,
}

impl Bus {
    fn new(sr: f32, cfg: &BusConfig) -> Bus {
        let ok = |v: Option<f32>| v.filter(|x| x.is_finite());
        let q = q_from_db(ok(cfg.q).unwrap_or(1.0));
        let filter = match (ok(cfg.hp), ok(cfg.lp)) {
            (Some(f), _) => Some((BiquadKind::Highpass, f)),
            (None, Some(f)) => Some((BiquadKind::Lowpass, f)),
            _ => None,
        }
        .map(|(kind, f)| { let b = Biquad::new(kind, sr, f, q, 0.0); [b.clone(), b] });
        Bus {
            cfg: cfg.clone(),
            gain: ok(cfg.gain).unwrap_or(1.0),
            filter,
            reverb: ok(cfg.reverb).map_or(0.0, |r| r.clamp(0.0, 1.0)),
        }
    }

    #[inline]
    fn process(&mut self, x: Stereo) -> Stereo {
        let x = x.scale(self.gain);
        match &mut self.filter {
            Some([l, r]) => Stereo { l: l.process(x.l), r: r.process(x.r) },
            None => x,
        }
    }
}

pub struct Master {
    sr: f32,
    /// App volume 0..1.5 (the JS `vol/100`).
    volume: f32,
    warmth: f32,
    duck_amount: f32,
    /// The `scGain` on the dry bus path, and how fast it returns to 1.
    duck_gain: f32,
    duck_coef: f32,
    buses: Vec<Bus>,
    room: Room,
    reverb: Reverb,
    delay: PingPong,
    /// SPACE / ECHO knob sends and master EQ gains (dB), live-adjustable.
    space_send: f32,
    echo_send: f32,
    eq_db: [f32; 3],
    hpf: [Biquad; 2],
    glue: Compressor,
    eq: [[Biquad; 3]; 2],
    sat: Saturator,
    limiter: Compressor,
    brick: Limiter,
}

/// A number that is safe to feed into feedback networks.
#[inline]
fn clean(x: Stereo) -> Stereo {
    let f = |v: f32| if v.is_finite() { v.clamp(-64.0, 64.0) } else { 0.0 };
    Stereo { l: f(x.l), r: f(x.r) }
}

impl Master {
    pub fn new(sr: f32) -> Master {
        let hpf = Biquad::highpass(sr, 28.0, q_from_db(0.7));
        let eq = [
            Biquad::new(BiquadKind::Lowshelf, sr, 120.0, 1.0, EQ_LOW_DB),
            Biquad::new(BiquadKind::Peaking, sr, 1000.0, 0.9, EQ_MID_DB),
            Biquad::new(BiquadKind::Highshelf, sr, 8000.0, 1.0, EQ_HIGH_DB),
        ];
        let room = Room::default();
        Master {
            sr,
            volume: 0.8,
            warmth: 0.3,
            duck_amount: 0.0,
            duck_gain: 1.0,
            duck_coef: 1.0,
            buses: Vec::new(),
            reverb: Reverb::new(sr, &room),
            room,
            delay: PingPong::new(sr),
            space_send: SPACE_SEND,
            echo_send: ECHO_SEND,
            eq_db: [EQ_LOW_DB, EQ_MID_DB, EQ_HIGH_DB],
            hpf: [hpf.clone(), hpf],
            glue: Compressor::new(sr, -14.0, 8.0, 3.5, 0.004, 0.22),
            eq: [eq.clone(), eq],
            sat: Saturator::default(),
            limiter: Compressor::new(sr, -2.0, 0.0, 20.0, 0.001, 0.09),
            brick: Limiter::new(sr, 0.97),
        }
    }

    /// Apply the song's mix directives (called whenever new code is loaded).
    /// Reverb and bus filters are only rebuilt when their settings change, so
    /// tails keep ringing across live edits.
    pub fn configure(&mut self, mix: &MixSettings) {
        self.warmth = mix.warmth.clamp(0.0, 1.0);
        self.duck_amount = mix.duck.clamp(0.0, 0.95);
        if mix.room != self.room {
            self.room = mix.room.clone();
            self.reverb = Reverb::new(self.sr, &self.room);
        }
        let same = self.buses.len() == mix.buses.len() && self.buses.iter().zip(&mix.buses).all(|(b, c)| b.cfg == *c);
        if !same {
            self.buses = mix.buses.iter().map(|c| Bus::new(self.sr, c)).collect();
        }
    }

    /// Bus for a voice's `| bus NAME` (0 = main). A name without a `bus:` line
    /// behaves exactly like the main bus (the JS makes a plain unity sub-bus).
    pub fn bus_index(&self, name: Option<&str>) -> usize {
        name.and_then(|n| self.buses.iter().position(|b| b.cfg.name.eq_ignore_ascii_case(n))).map_or(0, |i| i + 1)
    }
    pub fn bus_count(&self) -> usize { 1 + self.buses.len() }

    /// A kick just hit: pump the dry mix if `duck:` is set. `bar_sec` = bar length.
    /// JS `duckPump`: jump to `max(0.05, 1-duck)`, glide back with τ = `min(0.12, bar/8)`.
    pub fn duck(&mut self, bar_sec: f32) {
        if self.duck_amount <= 0.0 { return; }
        self.duck_gain = (1.0 - self.duck_amount).max(0.05);
        let tau = (bar_sec / 8.0).clamp(1e-3, 0.12);
        self.duck_coef = 1.0 - (-1.0 / (tau * self.sr)).exp();
    }

    pub fn set_volume(&mut self, v: f32) { self.volume = v.clamp(0.0, 1.5); }

    /// The SPACE knob, 0..100 (browser default 20): how much of the mix feeds the reverb.
    pub fn set_space(&mut self, knob: f32) { self.space_send = knob.clamp(0.0, 100.0) / 100.0 * 0.9; }
    /// The ECHO knob, 0..100 (browser default 15): how much of the mix feeds the ping-pong delay.
    pub fn set_echo(&mut self, knob: f32) { self.echo_send = knob.clamp(0.0, 100.0) / 100.0; }
    /// Master EQ in dB (low shelf 120 Hz, peak 1 kHz, high shelf 8 kHz), -18..18.
    pub fn set_eq(&mut self, low: f32, mid: f32, high: f32) {
        self.eq_db = [low, mid, high].map(|g| g.clamp(-18.0, 18.0));
        for ch in &mut self.eq {
            ch[0].set(BiquadKind::Lowshelf, self.sr, 120.0, 1.0, self.eq_db[0]);
            ch[1].set(BiquadKind::Peaking, self.sr, 1000.0, 0.9, self.eq_db[1]);
            ch[2].set(BiquadKind::Highshelf, self.sr, 8000.0, 1.0, self.eq_db[2]);
        }
    }

    pub fn process(&mut self, input: &MixInput) -> Stereo {
        // submixes feed the main bus (and, optionally, the reverb)
        let mut bus = clean(input.buses.first().copied().unwrap_or_default());
        let mut reverb_in = clean(input.reverb);
        for (i, b) in self.buses.iter_mut().enumerate() {
            let Some(x) = input.buses.get(i + 1) else { break };
            let y = b.process(clean(*x));
            bus.add(y);
            if b.reverb > 0.0 { reverb_in.add(y.scale(b.reverb)); }
        }
        // the whole bus feeds both effects at full level; voices add their sends
        reverb_in.add(bus);
        let mut delay_in = clean(input.delay);
        delay_in.add(bus);
        let space = self.reverb.process(reverb_in);
        let echo = self.delay.process(delay_in);

        self.duck_gain += (1.0 - self.duck_gain) * self.duck_coef;
        let mut mix = bus.scale(self.duck_gain);
        mix.add(echo.scale(self.echo_send));
        mix.add(space.scale(self.space_send));

        // mastering chain
        let mut x = mix.scale(self.volume * 0.8);
        x = Stereo { l: self.hpf[0].process(x.l), r: self.hpf[1].process(x.r) };
        x = self.glue.process(x, false);
        for (c, v) in self.eq.iter_mut().zip([&mut x.l, &mut x.r]) {
            *v = c.iter_mut().fold(*v, |s, f| f.process(s));
        }
        x = self.sat.process(x, self.warmth);
        x = self.limiter.process(x, false);
        let y = clean(self.brick.process(x));
        Stereo { l: y.l.clamp(-1.0, 1.0), r: y.r.clamp(-1.0, 1.0) }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dsp::Noise;

    const SR: f32 = 48000.0;

    fn input(bus: Stereo) -> MixInput { MixInput { buses: vec![bus], ..Default::default() } }

    /// Peak of the output from `from` to `to` seconds after a one-sample impulse.
    fn impulse_peaks(m: &mut Master, inp: impl Fn(Stereo) -> MixInput, windows: &[(f32, f32)]) -> Vec<f32> {
        let end = windows.iter().fold(0.0f32, |a, w| a.max(w.1));
        let out: Vec<Stereo> = (0..(end * SR) as usize).map(|i| m.process(&inp(Stereo::mono(if i == 0 { 0.8 } else { 0.0 })))).collect();
        windows.iter().map(|&(a, b)| out[(a * SR) as usize..(b * SR) as usize].iter().fold(0.0f32, |p, s| p.max(s.l.abs()).max(s.r.abs()))).collect()
    }

    #[test]
    fn reverb_and_delay_leave_tails() {
        // reverb only (the bus also feeds it, so send through the reverb input)
        let mut m = Master::new(SR);
        let rev = |s: Stereo| MixInput { buses: vec![Stereo::ZERO], reverb: s, ..Default::default() };
        let p = impulse_peaks(&mut m, rev, &[(0.05, 0.15), (0.5, 0.7), (3.0, 3.5)]);
        assert!(p[0] > 1e-3 && p[1] > 1e-5, "reverb tail {p:?}");
        assert!(p[2] < p[0] * 0.01, "and it decays {p:?}");

        // the ping-pong delay repeats every 0.2 s, alternating sides
        let mut m = Master::new(SR);
        let del = |s: Stereo| MixInput { buses: vec![Stereo::ZERO], delay: s, ..Default::default() };
        let out: Vec<Stereo> = (0..(1.0 * SR) as usize).map(|i| m.process(&del(Stereo::mono(if i == 0 { 0.8 } else { 0.0 })))).collect();
        // the chain's latency: the limiter's 128 samples of lookahead
        let lat = 128;
        let win = |t: f32| out[(t * SR) as usize + lat - 200..(t * SR) as usize + lat + 400].iter().fold(Stereo::ZERO, |a, s| Stereo { l: a.l.max(s.l.abs()), r: a.r.max(s.r.abs()) });
        let (first, second) = (win(0.2), win(0.4));
        assert!(first.l > 0.02 && first.l > first.r * 3.0, "first echo on the left: {first:?}");
        assert!(second.r > 0.005 && second.r > second.l * 3.0, "second on the right: {second:?}");
        assert!(win(0.3).l < first.l * 0.1, "nothing between repeats");
    }

    /// `cargo test --release -p loom-audio --lib -- bench_master --ignored --nocapture`
    #[test]
    #[ignore]
    fn bench_master() {
        for room in ["", "room: fdn"] {
            let mut m = Master::new(SR);
            m.configure(&MixSettings::parse(&format!("{room}\nbus: drums gain 0.8 hp 40")));
            let mut n = Noise::new(1);
            let t = std::time::Instant::now();
            let frames = 480000;
            for _ in 0..frames {
                let s = Stereo { l: n.sample() * 0.3, r: n.sample() * 0.3 };
                m.process(&MixInput { buses: vec![s, s], reverb: s.scale(0.2), delay: s.scale(0.1) });
            }
            println!("master {room:?}: {:.0} ns/sample", t.elapsed().as_nanos() as f64 / frames as f64);
        }
    }

    #[test]
    fn fdn_room_differs_from_schroeder() {
        let render = |src: &str| {
            let mut m = Master::new(SR);
            m.configure(&MixSettings::parse(src));
            let rev = |s: Stereo| MixInput { buses: vec![Stereo::ZERO], reverb: s, ..Default::default() };
            (0..(1.5 * SR) as usize).map(|i| m.process(&rev(Stereo::mono(if i == 0 { 0.8 } else { 0.0 }))).l).collect::<Vec<f32>>()
        };
        let (a, b) = (render(""), render("room: fdn 0.9 3000"));
        let diff: f32 = a.iter().zip(&b).map(|(x, y)| (x - y).abs()).sum();
        let late = |v: &[f32]| v[(1.0 * SR) as usize..].iter().map(|x| x * x).sum::<f32>();
        assert!(diff > 1.0, "different networks");
        assert!(late(&b) > late(&a) * 4.0, "a 0.9 FDN rings longer than the default Schroeder");
    }

    #[test]
    fn duck_dips_then_recovers() {
        // the same tone through a ducked and an unducked master: only the dry
        // bus pumps (the reverb and echo returns are not ducked, as in the JS graph)
        let (mut ducked, mut plain) = (Master::new(SR), Master::new(SR));
        ducked.configure(&MixSettings::parse("duck: 0.8"));
        plain.configure(&MixSettings::parse("duck: 0.8"));
        let tone = |i: usize| input(Stereo::mono(0.1 * (i as f32 * 0.03).sin()));
        let mut ratio = |from: usize, n: usize| {
            let (mut a, mut b) = (0.0f32, 0.0f32);
            for i in from..from + n {
                if i == 24000 { ducked.duck(2.0); }
                a = a.max(ducked.process(&tone(i)).l.abs());
                b = b.max(plain.process(&tone(i)).l.abs());
            }
            a / b
        };
        assert!((ratio(0, 24000) - 1.0).abs() < 1e-6);
        ratio(24000, 128); // the limiter's lookahead
        let dip = ratio(24128, 480);
        ratio(24608, 47392);
        let back = ratio(72000, 4800);
        assert!(dip < 0.8, "dips: {dip}"); // the undocked reverb return is loud on a sustained tone
        assert!(back > 0.98, "recovers: {back}");
    }

    #[test]
    fn bus_gain_and_routing() {
        let mut m = Master::new(SR);
        m.configure(&MixSettings::parse("bus: drums gain 0.25\nbus: pads hp 2000"));
        assert_eq!((m.bus_count(), m.bus_index(Some("drums")), m.bus_index(Some("pads")), m.bus_index(None), m.bus_index(Some("nope"))), (3, 1, 2, 0, 0));
        let level = |m: &mut Master, bus: usize, f: f32| {
            let mut inp = MixInput { buses: vec![Stereo::ZERO; 3], ..Default::default() };
            let mut peak = 0.0f32;
            for i in 0..24000 {
                inp.buses[bus] = Stereo::mono(0.05 * (i as f32 * f).sin());
                if i > 12000 { peak = peak.max(m.process(&inp).l.abs()); } else { m.process(&inp); }
            }
            peak
        };
        let main = level(&mut Master::new(SR), 0, 0.05);
        let drums = level(&mut m, 1, 0.05);
        assert!(drums < main * 0.4 && drums > main * 0.1, "gain 0.25: {main} vs {drums}");
        let low_on_pads = level(&mut m, 2, 2.0 * std::f32::consts::PI * 100.0 / SR);
        assert!(low_on_pads < main * 0.1, "hp 2000 removes 100 Hz: {low_on_pads}");
    }

    #[test]
    fn never_exceeds_full_scale() {
        let mut m = Master::new(SR);
        m.configure(&MixSettings::parse("warmth: 1\nroom: fdn 0.97 20000\nbus: x gain 2 lp 100 q 30 reverb 1\nbus: y lp nan q inf"));
        m.set_volume(1.5);
        let mut n = Noise::new(9);
        let mut peak = 0.0f32;
        for i in 0..(3.0 * SR) as usize {
            let loud = 4.0; // +12 dB
            let s = Stereo { l: n.sample() * loud, r: n.sample() * loud };
            let inp = MixInput { buses: vec![s, s, s], reverb: s, delay: s };
            if i % 24000 == 0 { m.duck(2.0); }
            let y = m.process(&inp);
            assert!(y.l.is_finite() && y.r.is_finite());
            peak = peak.max(y.l.abs()).max(y.r.abs());
        }
        assert!(peak <= 1.0 && peak > 0.5, "{peak}");
        // garbage in, silence-safe out
        let y = m.process(&MixInput { buses: vec![Stereo { l: f32::NAN, r: f32::INFINITY }], ..Default::default() });
        assert!(y.l.is_finite() && y.r.is_finite());
    }
}
