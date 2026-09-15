//! HUM — one take: a 3-2-1 countdown, then recording `Settings::seconds()`
//! of microphone audio (with a live level), then the analysis by
//! `loom_app::hum::melody`. The dialog shows [`HumTake::step`] each frame
//! and calls [`HumTake::tick`] to pull the microphone's samples.
//!
//! The microphone is behind the small [`Mic`] trait, so the whole flow is
//! tested with a fake one; the apps use `loom_audio::input::MicInput`.
//! Without a microphone the take simply says so (« pas de micro »).

use std::time::{Duration, Instant};

use loom_app::hum::{self, Hum, Settings};
use loom_audio::input::MicInput;

/// Seconds of countdown before recording.
pub const COUNTDOWN: f64 = 3.0;
/// If the microphone stops delivering, give up this long after the take should have ended.
pub const STALL: Duration = Duration::from_secs(2);

/// What a take needs from a microphone.
pub trait Mic {
    /// Samples captured since the last call (mono, -1..1).
    fn take(&self) -> Vec<f32>;
    /// Current input peak, 0..1.
    fn level(&self) -> f32;
    fn sample_rate(&self) -> u32;
}

impl Mic for MicInput {
    fn take(&self) -> Vec<f32> {
        MicInput::take(self)
    }
    fn level(&self) -> f32 {
        MicInput::level(self)
    }
    fn sample_rate(&self) -> u32 {
        MicInput::sample_rate(self)
    }
}

/// Where the take is.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Step<'a> {
    /// The microphone could not be opened (the error, in French).
    NoMic(&'a str),
    /// 3, 2, 1…
    Countdown(u32),
    /// Recording, 0..1 done.
    Recording(f32),
    /// A melody was heard.
    Heard(&'a Hum),
    /// Only silence (or noise) was heard.
    Silent(&'a Hum),
}

/// One HUM take.
pub struct HumTake<M: Mic = MicInput> {
    mic: Option<M>,
    error: Option<String>,
    settings: Settings,
    song: String,
    started: Instant,
    recorded: Vec<f32>,
    result: Option<Hum>,
}

impl<M: Mic> HumTake<M> {
    /// Start a take for `song` (its tempo and key shape the result).
    pub fn new(mic: Result<M, String>, song: &str, now: Instant) -> HumTake<M> {
        let (mic, error) = match mic {
            Ok(mic) => (Some(mic), None),
            Err(e) => (None, Some(format!("pas de micro : {e} — branche un micro, puis recommence"))),
        };
        HumTake { mic, error, settings: Settings::for_song(song), song: song.to_string(), started: now, recorded: Vec::new(), result: None }
    }

    /// `[ hum ] chante 16 temps… (4.0s)`.
    pub fn prompt(&self) -> String {
        self.settings.prompt()
    }

    /// The microphone level, 0..1 (0 once the take is over).
    pub fn level(&self) -> f32 {
        self.mic.as_ref().map_or(0.0, |m| m.level().clamp(0.0, 1.0))
    }

    /// Pull the microphone's samples: dropped during the countdown, kept
    /// while recording; once enough is recorded, analyse and close the microphone.
    pub fn tick(&mut self, now: Instant) {
        let Some(mic) = &self.mic else { return };
        let chunk = mic.take();
        let rate = mic.sample_rate().max(1);
        if now < self.started + Duration::from_secs_f64(COUNTDOWN) {
            return; // what was heard during 3-2-1 is not part of the song
        }
        self.recorded.extend(chunk);
        let wanted = self.wanted(rate);
        let deadline = self.started + Duration::from_secs_f64(COUNTDOWN + self.settings.seconds()) + STALL;
        if self.recorded.len() >= wanted || now >= deadline {
            self.recorded.truncate(wanted);
            self.result = Some(hum::melody(&self.recorded, rate as f32, &self.song));
            self.mic = None;
        }
    }

    /// Where the take is, to draw it.
    pub fn step(&self, now: Instant) -> Step<'_> {
        if let Some(error) = &self.error {
            return Step::NoMic(error);
        }
        if let Some(hum) = &self.result {
            return if hum.is_silent() { Step::Silent(hum) } else { Step::Heard(hum) };
        }
        let elapsed = now.saturating_duration_since(self.started).as_secs_f64();
        if elapsed < COUNTDOWN {
            return Step::Countdown((COUNTDOWN - elapsed).ceil() as u32);
        }
        let rate = self.mic.as_ref().map_or(1, |m| m.sample_rate().max(1));
        Step::Recording((self.recorded.len() as f32 / self.wanted(rate) as f32).clamp(0.0, 1.0))
    }

    /// The analysed take, once over.
    pub fn result(&self) -> Option<&Hum> {
        self.result.as_ref()
    }

    fn wanted(&self, rate: u32) -> usize {
        ((self.settings.seconds() * rate as f64) as usize).max(1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;

    const RATE: u32 = 8000;
    const SONG: &str = "bpm: 120\nkey: a minor\nkick: x . . .\n";

    #[derive(Default)]
    struct FakeMic(RefCell<Vec<f32>>);

    impl FakeMic {
        fn sing(&self, hz: f32, seconds: f64) {
            let n = (seconds * RATE as f64) as usize;
            let mut buf = self.0.borrow_mut();
            let offset = buf.len();
            buf.extend((0..n).map(|i| 0.5 * (std::f32::consts::TAU * hz * (i + offset) as f32 / RATE as f32).sin()));
        }
        fn silence(&self, seconds: f64) {
            self.0.borrow_mut().extend(std::iter::repeat_n(0.0, (seconds * RATE as f64) as usize));
        }
    }

    impl Mic for FakeMic {
        fn take(&self) -> Vec<f32> {
            std::mem::take(&mut *self.0.borrow_mut())
        }
        fn level(&self) -> f32 {
            0.4
        }
        fn sample_rate(&self) -> u32 {
            RATE
        }
    }

    fn at(start: Instant, seconds: f64) -> Instant {
        start + Duration::from_secs_f64(seconds)
    }

    #[test]
    fn countdown_then_recording_then_a_melody() {
        let t0 = Instant::now();
        let mut take = HumTake::new(Ok(FakeMic::default()), SONG, t0);
        assert_eq!(take.prompt(), "[ hum ] chante 16 temps… (4.0s)");
        assert_eq!(take.step(t0), Step::Countdown(3));
        assert_eq!(take.step(at(t0, 2.5)), Step::Countdown(1));
        // noise during the countdown is thrown away
        take.mic.as_ref().unwrap().sing(1000.0, 2.9);
        take.tick(at(t0, 2.9));
        assert!((take.level() - 0.4).abs() < 1e-6);
        // A4 = 440 Hz = degree 1 of A minor, in 40 small chunks
        for i in 1..=40 {
            take.mic.as_ref().unwrap().sing(440.0, 0.1);
            let now = at(t0, 3.0 + i as f64 * 0.1);
            take.tick(now);
            if i == 20 {
                match take.step(now) {
                    Step::Recording(p) => assert!((p - 0.5).abs() < 0.01, "{p}"),
                    other => panic!("{other:?}"),
                }
            }
        }
        let Step::Heard(hum) = take.step(at(t0, 7.1)) else { panic!("{:?}", take.step(at(t0, 7.1))) };
        assert!(hum.notes.iter().all(|n| *n == Some(69)), "{:?}", hum.notes);
        assert!(hum.line.starts_with("hum: 1"), "{}", hum.line);
        assert!(hum.tokens.iter().all(|t| *t == hum.tokens[0]));
        assert_eq!(take.level(), 0.0, "the microphone is closed");
        let song = hum::append(SONG, &hum.line);
        assert_eq!(loom_core::Program::parse(&song).voices.len(), 2);
    }

    #[test]
    fn silence_and_a_stalled_microphone() {
        let t0 = Instant::now();
        let mut take = HumTake::new(Ok(FakeMic::default()), SONG, t0);
        take.tick(at(t0, 3.1));
        take.mic.as_ref().unwrap().silence(1.0);
        take.tick(at(t0, 4.1));
        assert!(matches!(take.step(at(t0, 4.1)), Step::Recording(_)));
        // no more audio arrives: the take ends after the stall delay, with what it has
        take.tick(at(t0, 3.0 + 4.0 + 2.1));
        let Step::Silent(hum) = take.step(at(t0, 9.2)) else { panic!() };
        assert!(hum.message().contains("rien entendu"));
    }

    #[test]
    fn no_microphone() {
        let t0 = Instant::now();
        let mut take: HumTake<FakeMic> = HumTake::new(Err("aucun micro trouvé".into()), SONG, t0);
        take.tick(at(t0, 5.0));
        match take.step(at(t0, 5.0)) {
            Step::NoMic(text) => assert!(text.starts_with("pas de micro : aucun micro trouvé"), "{text}"),
            other => panic!("{other:?}"),
        }
        assert!(take.result().is_none());
        assert_eq!(take.level(), 0.0);
    }
}
