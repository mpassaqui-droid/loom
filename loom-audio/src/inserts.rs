//! The per-note insert chain — the port of the browser's `tone()` EFFECTS half,
//! everything after the amplitude envelope, in the same order:
//!
//! vocode → eq → vowel → drive → fold → comp → crush → redux → reson → flange →
//! width → ring → am → phaser → lfo gain → lfo pan → pan → orbit → tgate →
//! [echo tap → bus] → haas → autopan → ir → out ; chorus taps `out` → bus.
//!
//! Only the effects a voice actually uses are built, so a plain voice costs
//! almost nothing. While the signal is still mono (L == R) the per-channel
//! stages compute one side only.
//!
//! - [`color`]: tone-shaping stages (eq … phaser).
//! - [`motion`]: level and position (tremolo, pans, orbit, tgate, haas, autopan).
//! - [`space`]: echo, chorus and the `| ir` reverb.
//! - [`vocoder`]: the 12-band channel vocoder.
//! - [`prims`]: small helpers also used by the master (pan law, delay rings…).

mod color;
mod motion;
pub(crate) mod prims;
mod space;
mod vocoder;

use crate::clock::{name_salt, Clock};
use crate::note::{NoteOut, NoteSpec, Stereo};
use crate::sample::SampleBank;
use loom_core::Param;

/// One effect in a voice's chain.
trait Stage: Send {
    /// Called once, just before the first sample (resolve `A..B` sweeps at note start, like the JS `pvVal(…, t)`).
    fn start(&mut self, _clock: &Clock) {}
    /// Process one sample. `mono` = the signal is still one channel duplicated;
    /// per-channel stages may then compute only the left side.
    fn process(&mut self, x: Stereo, mono: bool) -> Stereo;
    /// True if the output is a real stereo signal whatever the input (panners, haas, ir).
    fn makes_stereo(&self) -> bool { false }
    /// Seconds this stage keeps ringing once its input is silent.
    fn tail(&self) -> f32 { 0.0 }
    /// Longest silence this stage can output while still ringing (a delay's length, a gate's off time).
    fn gap(&self) -> f32 { 0.0 }
}

/// Output quieter than this (both sides, both taps) counts as silence (-86 dBFS).
const SILENCE: f32 = 5e-5;

pub struct Inserts {
    /// vocode … tgate: everything before the echo tap.
    pre: Vec<Box<dyn Stage>>,
    echo: Option<space::Echo>,
    /// haas, autopan, ir.
    post: Vec<Box<dyn Stage>>,
    /// Index of the first stage (counting `pre` then `post`) that makes the signal stereo.
    first_stereo: usize,
    chorus: Option<space::Chorus>,
    started: bool,
    /// Has the source ever produced a different L and R?
    src_stereo: bool,
    tail: f32,
    /// Silent samples in a row, and how many mean "nothing is ringing any more".
    quiet: u32,
    quiet_limit: u32,
}

impl Inserts {
    pub fn new(spec: &NoteSpec, bank: &SampleBank) -> Inserts {
        let s = spec.synth;
        let sr = spec.sr;
        let salt = name_salt(spec.name);
        let mut pre: Vec<Box<dyn Stage>> = Vec::new();
        let mut post: Vec<Box<dyn Stage>> = Vec::new();

        if let Some(data) = s.vocode.as_deref().and_then(|n| bank.get(n)) {
            if !data.channels.is_empty() && data.channels[0].len() > 1 {
                pre.push(Box::new(vocoder::Vocoder::new(data.clone(), sr)));
            }
        }
        if let Some(eq) = s.eq { pre.push(Box::new(color::Eq::new(eq, sr))); }
        if let Some(v) = s.vowel { pre.push(Box::new(color::Vowel::new(v, sr))); }
        if let Some(d) = s.drive.filter(|d| *d != Param::Const(0.0)) { pre.push(Box::new(color::Drive::new(d, s.over, salt))); }
        if let Some(f) = s.fold.filter(|f| *f > 0.0) { pre.push(Box::new(color::Fold::new(f))); }
        if let Some(c) = s.comp.filter(|c| *c > 0.0) { pre.push(Box::new(color::Comp::new(c, sr))); }
        if let Some(b) = s.crush { pre.push(Box::new(color::Crush::new(b))); }
        if let Some(r) = s.redux.filter(|r| *r > 0.0) { pre.push(Box::new(color::Redux::new(r))); }
        if let Some(r) = s.reson { pre.push(Box::new(color::Reson::new(r, sr))); }
        if let Some(f) = s.flange { pre.push(Box::new(color::Flange::new(f, sr))); }
        if let Some(w) = s.width.filter(|w| *w > 0.0) { pre.push(Box::new(color::Width(w))); }
        if let Some(hz) = s.ring { pre.push(Box::new(color::RingMod::new(hz, sr))); }
        if let Some(hz) = s.am { pre.push(Box::new(color::Am::new(hz, sr))); }
        if let Some(p) = s.phaser.filter(|p| *p > 0.0) { pre.push(Box::new(color::Phaser::new(p, sr))); }

        let lfos = |targets: &[&str]| -> Vec<&loom_core::Lfo> {
            [&s.lfo, &s.lfo2].into_iter().flatten().filter(|l| targets.contains(&l.target.as_str())).collect()
        };
        let trem = lfos(&["gain", "vol"]);
        if !trem.is_empty() { pre.push(Box::new(motion::Tremolo::new(&trem, sr))); }
        let lpan = lfos(&["pan"]);
        if !lpan.is_empty() { pre.push(Box::new(motion::LfoPan::new(&lpan, sr))); }
        if let Some(p) = spec.pan.map(Param::Const).or(s.pan) { pre.push(Box::new(motion::Pan::new(p, s.over, salt))); }
        if let Some(rate) = s.orbit { pre.push(Box::new(motion::Orbit::new(rate, sr))); }
        if let Some(n) = s.tgate { pre.push(Box::new(motion::TGate::new(n, spec.bar_sec, sr))); }

        let echo = s.echo.map(|e| space::Echo::new(e, spec.bar_sec, sr));

        if let Some(h) = s.haas.filter(|h| *h > 0.0) { post.push(Box::new(motion::Haas::new(h, sr))); }
        if let Some(rate) = s.autopan { post.push(Box::new(motion::AutoPan::new(rate, sr))); }
        if let Some(kind) = &s.ir { post.push(Box::new(space::IrVerb::new(kind, s.irmix.unwrap_or(0.35), sr))); }

        let chorus = s.chorus.filter(|c| *c > 0.0).map(|c| space::Chorus::new(c, sr));

        // how long the chain rings, and the longest silent stretch inside that ringing
        let stages = pre.iter().chain(post.iter());
        let (mut tail, mut gap) = stages.fold((0.0, 0.0), |(t, g), st| (t + st.tail(), g + st.gap()));
        if let Some(e) = &echo { tail += e.tail(); gap += e.gap(); }
        if let Some(c) = &chorus { tail += c.tail(); gap += c.gap(); }
        let quiet_limit = ((gap + 0.05) * sr) as u32;
        let first_stereo = pre.iter().chain(post.iter()).position(|st| st.makes_stereo()).unwrap_or(usize::MAX);

        Inserts { pre, echo, post, first_stereo, chorus, started: false, src_stereo: false, tail, quiet: 0, quiet_limit }
    }

    /// Seconds the chain keeps ringing after its input goes silent (echo, reson…).
    /// Drops to 0 once the output has been silent for longer than any delay in the
    /// chain could hold a sound, so faded tails stop costing CPU early.
    pub fn tail(&self) -> f32 {
        if self.quiet > self.quiet_limit { 0.0 } else { self.tail }
    }

    pub fn process(&mut self, input: Stereo, clock: &Clock) -> NoteOut {
        if self.pre.is_empty() && self.post.is_empty() && self.echo.is_none() && self.chorus.is_none() {
            return NoteOut { main: input, bus_only: Stereo::ZERO };
        }
        if !self.started {
            self.started = true;
            self.pre.iter_mut().chain(self.post.iter_mut()).for_each(|st| st.start(clock));
        }
        self.src_stereo |= input.l != input.r;
        // with a mono source, stages up to (and including) the first panner get a mono signal
        let mono_count = if self.src_stereo { 0 } else { self.first_stereo.saturating_add(1) };
        let mut x = input;
        for (i, st) in self.pre.iter_mut().enumerate() {
            x = st.process(x, i < mono_count);
        }
        let n = self.pre.len();
        let mut bus_only = match &mut self.echo {
            Some(e) => e.process(x, n < mono_count),
            None => Stereo::ZERO,
        };
        for (i, st) in self.post.iter_mut().enumerate() {
            x = st.process(x, n + i < mono_count);
        }
        let mono = n + self.post.len() < mono_count;
        if let Some(c) = &mut self.chorus { bus_only.add(c.process(x, mono)); }

        let loud = x.l.abs().max(x.r.abs()).max(bus_only.l.abs()).max(bus_only.r.abs());
        if loud > SILENCE { self.quiet = 0 } else { self.quiet = self.quiet.saturating_add(1) }
        NoteOut { main: x, bus_only }
    }
}

#[cfg(test)]
mod tests;
